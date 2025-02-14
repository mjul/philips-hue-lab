use clap::{Arg, Command};
use reqwest::blocking;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::io::ErrorKind::HostUnreachable;

const HUE_API_APP_NAME: &str = "philips_hue_lab";
const HUE_API_USER_NAME: &str = "hue_lab_user";

/// The Hue Bridge root CA.
///
/// See documentation at
/// <https://developers.meethue.com/develop/application-design-guidance/using-https/>
const HUE_ROOT_CA: &str = include_str!("../resources/huebridge_cacert.pem");

/// IP Address of the Hue Bridge
struct BridgeIp(String);

#[derive(Deserialize, Debug)]
struct BridgeKey {
    #[serde(rename = "username")]
    user_name: String,
    #[serde(rename = "clientkey")]
    client_key: String,
}

#[derive(Debug)]
struct HueError(String, Option<Box<dyn Error>>);
impl std::fmt::Display for HueError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.1 {
            None => write!(f, "{}", self.0),
            Some(e) => write!(f, "{} :: {}", self.0, *e),
        }
    }
}
impl Error for HueError {}

/// The body for the POST /api endpoint (create a user)
#[derive(Serialize, Debug)]
struct CreateUserRequestBody {
    #[serde(rename = "devicetype")]
    device_type: String,
}
impl CreateUserRequestBody {
    fn from(app_name: &str, user_name: &str) -> Self {
        CreateUserRequestBody {
            device_type: format!("{}#{}", app_name, user_name),
        }
    }
}

fn create_key(bridge_ip: &BridgeIp) -> Result<BridgeKey, HueError> {
    let body = CreateUserRequestBody::from(HUE_API_APP_NAME, HUE_API_USER_NAME);
    let response =
        post_request(&bridge_ip, "/api", &body).map_err(|e| HueError(e.to_string(), Some(e)))?;
    let bridge_key = serde_json::from_value::<BridgeKey>(response)
        .map_err(|e| HueError(e.to_string(), Some(Box::new(e))))?;
    Ok(bridge_key)
}

fn post_request<T>(
    bridge_ip: &BridgeIp,
    path: &str,
    body: &T,
) -> Result<serde_json::Value, Box<dyn Error>>
where
    T: ?Sized + Serialize,
{
    let url = format!("https://{}{}", bridge_ip.0, path);
    println!("Requesting: {}", url);
    let cert = reqwest::Certificate::from_pem(HUE_ROOT_CA.as_bytes())?;
    let client = blocking::ClientBuilder::new()
        .add_root_certificate(cert)
        // otherwise we get an error  "The certificate's CN name does not match the passed value."
        .danger_accept_invalid_certs(true)
        .build()?;
    let body_str = serde_json::to_string(body)?;
    println!("Body: {:?}", body_str);
    let response = client
        .post(&url)
        .header("Accept", "application/json")
        .body(body_str)
        .send();
    println!("Raw response: {:?}", response);
    let result = response?.json::<serde_json::Value>()?;
    Ok(result)
}

fn main() -> Result<(), Box<dyn Error>> {
    let matches = Command::new("philips_hue_lab")
        .version(env!("CARGO_PKG_VERSION"))
        .about("Experimental CLI tools for Philips Hue ZigBee IoT devices.")
        .arg(
            Arg::new("bridge")
                .long("bridge")
                .value_name("IP")
                .help("The IP address of the Hue Bridge. You can find the IP number by opening the Philips Hue app, selecting the Hue Bridge, and pressing the information icon.")
                .num_args(1),
        )
        .subcommand(
            Command::new("create-key")
                .about("Ask the Hue Bridge to generate an application key. Press the Link button on the bridge to authorize this operation.")
        )
        .get_matches();

    let mut bridge = None;
    if let Some(bridge_ip) = matches.get_one::<String>("bridge") {
        println!("Using Hue Bridge at: {}", bridge_ip);
        bridge = Some(BridgeIp(String::from(bridge_ip)));
    } else {
        println!("No Hue Bridge IP address provided.");
    }

    if let Some(_sub_matches) = matches.subcommand_matches("create-key") {
        println!("Requesting creation of a new application key on the Hue Bridge. Make sure you have pressed the link button on the bridge!");
        match create_key(&bridge.unwrap()) {
            Ok(bridge_key) => {
                println!("Key created: {:?}", bridge_key);
                Ok(())
            }
            Err(e) => Err(Box::new(HueError(
                format!("Error creating key: {:?}", e.0),
                e.1,
            ))),
        }
    } else {
        Err(Box::new(HueError(
            String::from("No subcommand provided. Please provide a subcommand."),
            None,
        )))
    }
}
