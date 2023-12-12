mod profile;

use clap::{Parser, Subcommand};
use profile::{get_default_profile, load_profile, print_profiles};
use serde::{Deserialize, Serialize};
use serde_json;
use std::io::prelude::*;
use std::path::PathBuf;

use base64::{engine::general_purpose, Engine as _};
use reqwest::blocking::{Client, Response};
use reqwest::{header, StatusCode};
use std::fs::File;
use std::process::exit;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Sets the profile
    #[arg(short, long, value_name = "NAME", global = true)]
    profile: Option<String>,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Download the images of a document
    Images {
        /// Id of the document
        #[arg(short, long)]
        document_id: u64,
    },
    /// Download the source file of a document
    File {
        /// Id of the document
        #[arg(short, long)]
        document_id: u64,
    },
    /// Download the OCR tokens of a document
    Tokens {
        /// Id of the document
        #[arg(short, long)]
        document_id: u64,
    },
    /// Upload document
    Upload {
        /// File path
        file_path: PathBuf,

        /// Allowed document types
        #[arg(short, long, value_delimiter = ',', num_args = 1..)]
        classification_scope: Option<Vec<String>>,
    },
    Document {
        #[command(subcommand)]
        command: Option<DocumentCommands>,
    },
    Config {
        #[command(subcommand)]
        command: Option<ConfigCommands>,
    },
}

#[derive(Subcommand)]
enum DocumentCommands {
    /// List documents
    List { document_ids: Vec<String> },
}

#[derive(Subcommand)]
enum ConfigCommands {
    /// List configured profiles
    List,
}

fn main() {
    let cli = Cli::parse();

    let profile = match cli.profile.as_deref() {
        Some(pname) => load_profile(pname),
        None => get_default_profile(),
    };

    let api_client = ApiClient::new(format!("https://{}", profile.domain), &profile.api_token);

    // You can check for the existence of subcommands, and if found use their
    // matches just as you would the top level cmd
    match cli.command {
        Some(Commands::Upload {
            file_path,
            classification_scope,
        }) => api_client.upload_document(file_path, classification_scope),
        Some(Commands::Images { document_id }) => api_client.get_images(document_id),
        Some(Commands::File { document_id }) => api_client.get_source_files(document_id),
        Some(Commands::Tokens { document_id }) => api_client.get_tokens(document_id),
        Some(Commands::Document { command }) => match command {
            Some(DocumentCommands::List { document_ids }) => {
                api_client.list_documents(document_ids)
            }
            None => {}
        },
        Some(Commands::Config { command }) => match command {
            Some(ConfigCommands::List) => print_profiles(),
            None => {}
        },
        None => {}
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct ResourceRequest<A> {
    data: Resource<A>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct ResourceResponse<A> {
    data: Resource<A>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct ResourceArrayResponse<A> {
    data: Vec<Resource<A>>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Resource<A> {
    #[serde(rename = "type")]
    type_: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<String>,

    attributes: A,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct FileAttributes {
    url: String,
    mime_type: String,
    file_type: String,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct TextAttributes {
    value: String,
    confidence: Option<f64>,
    coordinates: Rectangle,
    page_id: String,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct DocumentAttributes {
    tenant_id: String,
	status: String,
	workflow_step: String,
	workflow_status: String,
	validation_required: bool,
	not_for_training: bool,
	created_at: String,
	updated_at: String,
	document_type_identifier: String,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct DocumentCreateAttributes {
    classification_scope: Option<Vec<String>>,
    files: Vec<DocumentFile>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct DocumentFile {
    file_name: String,
    base64_file: String,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Rectangle {
    top: f64,
    bottom: f64,
    left: f64,
    right: f64,
}

fn download_file(url: &str, filename: &str) {
    let data = reqwest::blocking::get(url).unwrap().bytes().unwrap();
    let mut file = File::create(filename).unwrap();
    file.write_all(&data).unwrap();
}

struct ApiClient {
    base_url: String,
    client: reqwest::blocking::Client,
}

fn extension_by_mime_type(mime_type: &str) -> String {
    match mime_type {
        "image/jpeg" => ".jpeg".to_string(),
        "application/pdf" => ".pdf".to_string(),
        _ => "".to_string(),
    }
}

impl ApiClient {
    fn new(base_url: String, token: &String) -> ApiClient {
        let mut headers = header::HeaderMap::new();
        headers.insert(
            "Content-Type",
            header::HeaderValue::from_static("application/vnd.api+json"),
        );

        let mut auth_value = header::HeaderValue::from_str(token).unwrap();
        auth_value.set_sensitive(true);
        headers.insert(header::AUTHORIZATION, auth_value);

        ApiClient {
            base_url: base_url,
            client: Client::builder().default_headers(headers).build().unwrap(),
        }
    }

    fn get(&self, url: String) -> Response {
        let response = match self.client.get(url).send() {
            Ok(res) => res,
            Err(err) => {
                println!("Unable to send request.\n\n{}", err);
                exit(1);
            }
        };

        if response.status() != StatusCode::OK {
            println!("Request failed with status code {}.", response.status());
            exit(1);
        }

        return response;
    }

    fn get_images(&self, document_id: u64) {
        self.get_files("color_jpeg", document_id);
    }

    fn get_source_files(&self, document_id: u64) {
        self.get_files("input_file", document_id);
    }

    fn get_files(&self, file_type: &str, document_id: u64) {
        let response = self.get(format!(
            "{}/v2/files/?filter[record_id]={}&extra_fields[files]=url",
            self.base_url, document_id
        ));

        let content: ResourceArrayResponse<FileAttributes> = match response.json() {
            Ok(payload) => payload,
            Err(err) => {
                println!("Unable to parse response. {}", err);
                exit(1);
            }
        };

        if content.data.is_empty() {
            println!("Document {} does not exist.", document_id);
            exit(1);
        }

        for (page_nr, file) in content
            .data
            .iter()
            .filter(|f| f.attributes.file_type == file_type)
            .enumerate()
        {
            let extension = extension_by_mime_type(&file.attributes.mime_type);
            let file_name = format!("{}-{}{}", document_id, page_nr, extension);
            println!("Downloading {}", file_name);
            download_file(&file.attributes.url, &file_name);
        }
    }

    fn get_tokens(&self, document_id: u64) {
        let response = self.get(format!(
            "{}/v2/documents/{}/recognitions",
            self.base_url, document_id
        ));

        let content: ResourceArrayResponse<TextAttributes> = match response.json() {
            Ok(payload) => payload,
            Err(err) => {
                println!("Unable to parse response. {}", err);
                exit(1);
            }
        };

        if content.data.is_empty() {
            println!("Document {} does not exist.", document_id);
            exit(1);
        }

        println!(
            "{: <30} {:.8} {:.8} {:.8} {:.8} {:.8}",
            "value", "confidence", "top", "bottom", "left", "right"
        );

        for tk in content.data.iter() {
            println!(
                "{: <30} {:.8} {:.8} {:.8} {:.8} {:.8}",
                tk.attributes.value,
                tk.attributes.confidence.unwrap_or(0.0),
                tk.attributes.coordinates.top,
                tk.attributes.coordinates.bottom,
                tk.attributes.coordinates.left,
                tk.attributes.coordinates.right,
            );
        }
    }

    fn upload_document(&self, file_path: PathBuf, classification_scope: Option<Vec<String>>) {
        let file_name = file_path
            .file_name()
            .expect("Unable to determine file name.")
            .to_os_string()
            .into_string()
            .unwrap();

        let mut file = File::open(&file_path).expect("Can't open file.");
        let mut buffer: Vec<u8> = Vec::new();
        file.read_to_end(&mut buffer).expect("Unable to read file.");

        let encoded = general_purpose::STANDARD.encode(&buffer);

        let payload = ResourceRequest {
            data: Resource {
                id: None,
                type_: "documents".to_string(),
                attributes: DocumentCreateAttributes {
                    classification_scope,
                    files: vec![DocumentFile {
                        file_name: file_name,
                        base64_file: encoded,
                    }],
                },
            },
        };
        let body = serde_json::to_string(&payload).expect("Unable to serialize request.");

        let url = format!("{}/v2/documents/", self.base_url);
        let response = match self.client.post(&url).body(body).send() {
            Ok(res) => res,
            Err(err) => {
                println!("Unable to send request.\n\n{}", err);
                exit(1);
            }
        };

        if response.status() != StatusCode::CREATED {
            println!("Request failed with status code {}.", response.status());
            exit(1);
        }

        let response_data: ResourceResponse<DocumentAttributes> =
            response.json().expect("Unable to parse API response.");
        println!(
            "Upload document {} to tenant {}.",
            response_data.data.id.unwrap(),
            response_data.data.attributes.tenant_id
        );
    }

    fn list_documents(&self, document_ids: Vec<String>) {
        let url = format!(
            "{}/v2/documents/?filter[id][eq]={}",
            self.base_url,
            document_ids.join(",")
        );
        let response = self.get(url);
        let content: ResourceArrayResponse<DocumentAttributes> = match response.json() {
            Ok(payload) => payload,
            Err(err) => {
                println!("Unable to parse response. {}", err);
                exit(1);
            }
        };

        for doc in content.data.iter() {
			println!("{}", &doc.id.unwrap());
		}
        // for doc in content.data.iter() {
        //     if let Some(doc_id) = &doc.id {
        //         println!("{}", doc_id);
        //     }
        // }
    }
}
