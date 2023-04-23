mod profile;

use profile::{load_profile, get_default_profile, print_profiles};
use clap::{Parser, Subcommand};
use serde::{Serialize, Deserialize};
use std::io::prelude::*;

use reqwest::blocking::Client;
use reqwest::{StatusCode, header};
use std::process::exit;
use std::fs::File;


#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
	/// Sets the profile
    #[arg(short, long, value_name = "NAME", global=true)]
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
	Config {
		#[command(subcommand)]
		command: Option<ConfigCommands>,
	}
}

#[derive(Subcommand)]
enum ConfigCommands {
	/// List configured profiles
    List
}

fn main() {
    let cli = Cli::parse();

    let profile = match cli.profile.as_deref() {
		Some(pname) => load_profile(pname),
		None => get_default_profile(),
	};

	let api_client = ApiClient::new(
		format!("https://{}", profile.domain), &profile.api_token
	);

    // You can check for the existence of subcommands, and if found use their
    // matches just as you would the top level cmd
    match &cli.command {
        Some(Commands::Images { document_id }) => api_client.get_images(*document_id),
        Some(Commands::File { document_id }) => api_client.get_source_files(*document_id),
        Some(Commands::Tokens { document_id }) => api_client.get_tokens(*document_id),
        Some(Commands::Config { command }) => {
			match command {
				Some(ConfigCommands::List) => print_profiles(),
				None => {}
			}
		}
        None => {}
    }
}


#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct FileResource {
	url: String,
	mime_type: String,
	file_type: String,
}


#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Rectangle {
	top: f64,
	bottom: f64,
	left: f64,
	right: f64,
}


#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct TextResource {
	value: String,
	confidence: Option<f64>,
	coordinates: Rectangle,
	page_id: String,
}


#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Resource<R>  {
	id: String,
	attributes: R,
}


#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct JsonApiResponse<R> {
	data: Vec<Resource<R>>,
}


fn download_file(url: &str, filename: &str) {
	let data = reqwest::blocking::get(url).unwrap().bytes().unwrap();
	let mut file = File::create(filename).unwrap();
	file.write_all(&data).unwrap();
}


struct ApiClient {
	base_url: String,
	client: reqwest::blocking::Client
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
		headers.insert("Content-Type", header::HeaderValue::from_static("application/json"));
		headers.insert("Accept", header::HeaderValue::from_static("application/json"));

		let mut auth_value = header::HeaderValue::from_str(token).unwrap();
		auth_value.set_sensitive(true);
		headers.insert(header::AUTHORIZATION, auth_value);

		ApiClient {
			base_url: base_url,
			client: Client::builder()
				.default_headers(headers)
				.build()
				.unwrap(),
		}
	}

	fn get_images(&self, document_id: u64) {
		self.get_files("color_jpeg", document_id);
	}

	fn get_source_files(&self, document_id: u64) {
		self.get_files("input_file", document_id);
	}

	fn get_files(&self, file_type: &str, document_id: u64) {
		let url = format!("{}/v2/files/?filter[record_id]={}&extra_fields[files]=url", self.base_url, document_id);
		let response = match self.client.get(url).send() {
			Ok(res) => res,
			Err(err) => {
				println!("Unable to retrieve document {}.\n\n{}", document_id, err);
				exit(1);
			}
		};

		if response.status() != StatusCode::OK {
			println!("Request failed with status code {}.", response.status());
			exit(1);
		}

		let content: JsonApiResponse<FileResource> = match response.json() {
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

		for (page_nr, file) in content.data.iter().filter(|f| f.attributes.file_type == file_type).enumerate() {
			let extension = extension_by_mime_type(&file.attributes.mime_type);
			let file_name = format!("{}-{}{}", document_id, page_nr, extension);
			println!("Downloading {}", file_name);
			download_file(&file.attributes.url, &file_name);
		};
	}


	fn get_tokens(&self, document_id: u64) {
		let url = format!("{}/v2/documents/{}/recognitions", self.base_url, document_id);
		let response = match self.client.get(url).send() {
			Ok(res) => res,
			Err(err) => {
				println!("Unable to retrieve document {}.\n\n{}", document_id, err);
				exit(1);
			}
		};

		if response.status() != StatusCode::OK {
			println!("Request failed with status code {}.", response.status());
			exit(1);
		}

		let content: JsonApiResponse<TextResource> = match response.json() {
			Ok(payload) => payload,
			Err(err) => {
				println!("Unable to parse response. {}", err);
				exit(1);
			}
		};

		for tk in content.data.iter() {
			println!("{}", tk.attributes.value);
		};
	}
}
