use home;
use serde::{Deserialize, Serialize};
use serde_yaml;
use std::fs;
use std::path::PathBuf;
use std::process::exit;

fn home_dir() -> PathBuf {
    let mut path = match home::home_dir() {
        Some(path) => path,
        None => {
            println!("Unable to determine home directory.");
            exit(1)
        }
    };
    path.push(".parashift/pp.yaml");
    return path;
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Profile {
    pub name: String,
    pub api_token: String,
    pub domain: String,
    pub tenant_id: Option<String>,
    pub default: bool,
}

impl Default for Profile {
    fn default() -> Self {
        Profile {
            name: String::from("default"),
            api_token: String::from("secret"),
            domain: String::from("api.parashift.io"),
            tenant_id: None,
            default: false,
        }
    }
}

pub fn load_profile(name: &str) -> Profile {
    for profile in load_config().profiles.into_iter() {
        if profile.name == name {
            return profile;
        }
    }
    println!("No profile with name \"{}\"", name);
    exit(1);
}

pub fn get_default_profile() -> Profile {
    for profile in load_config().profiles.into_iter() {
        if profile.default {
            return profile;
        }
    }
    println!("No default profile defined.");
    exit(1);
}

pub fn print_profiles() {
    for profile in load_config().profiles.into_iter() {
        println!("{} {} {}", profile.name, profile.domain, profile.api_token);
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Config {
    pub profiles: Vec<Profile>,
}

pub fn load_config() -> Config {
    let contents = fs::read_to_string(home_dir()).expect("Unable to read config file.");
    serde_yaml::from_str(&contents).unwrap()
}
