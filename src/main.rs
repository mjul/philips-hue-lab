use clap::{Arg, Command};
use reqwest::blocking;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::error::Error;
use std::fmt::{Display, Formatter};

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

/// App key for the Hue API
struct AppKey(String);
impl From<&AppKey> for String {
    fn from(key: &AppKey) -> Self {
        key.0.clone()
    }
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
    let parsed = parse_create_key_response(&response)?;
    Ok(BridgeKey {
        user_name: HUE_API_USER_NAME.to_string(),
        client_key: parsed.user_name,
    })
}

fn parse_create_key_response(
    response: &serde_json::Value,
) -> Result<HueApiCreateKeySuccessDetails, HueError> {
    let errors = parse_api_response_errors(&response);
    match (errors.is_empty(), response.is_array()) {
        (false, _) => {
            let inner: Option<Box<dyn Error>> = errors
                .into_iter()
                .next()
                .map(|e| Box::new(e) as Box<dyn Error>);
            Err(HueError(String::from("Could not create key."), inner))
        }
        (true, true) => {
            let success_details = response
                .as_array()
                .unwrap()
                .get(0)
                .unwrap()
                .as_object()
                .unwrap()
                .get("success");
            match success_details {
                None => Err(HueError(
                    String::from(
                        "Could not create key. success element not found in response array.",
                    ),
                    None,
                )),
                Some(details_json) => {
                    let result = serde_json::from_value::<HueApiCreateKeySuccessDetails>(
                        details_json.clone(),
                    )
                    .map_err(|e| HueError(e.to_string(), Some(Box::new(e))))?;
                    Ok(result)
                }
            }
        }
        // We don't expect this to be reachable under normal operation
        (_, _) => unimplemented!(),
    }
}

/// This is the API wire format of the Hue response for a successful create-key operation.
#[derive(Deserialize, Debug, PartialEq)]
struct HueApiCreateKeySuccessDetails {
    #[serde(rename = "username")]
    user_name: String,
}

/// This is the API wire format of the Hue Error message details.
#[derive(Deserialize, Debug, PartialEq)]
struct HueApiErrorMessage {
    #[serde(rename = "type")]
    type_value: i64,
    address: String,
    description: String,
}

impl Display for HueApiErrorMessage {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("{:?}", self))
    }
}

impl Error for HueApiErrorMessage {}

/// Parse and extract all API response errors.
/// Returns an empty vec if there are no errors in the response.
fn parse_api_response_errors(response: &serde_json::Value) -> Vec<HueApiErrorMessage> {
    match response.is_array() {
        true => response
            .as_array()
            .unwrap()
            .iter()
            .filter_map(
                |element| match (element.is_object(), element.get("error")) {
                    (true, Some(details)) => {
                        let msg =
                            serde_json::from_value::<HueApiErrorMessage>(details.clone()).unwrap();
                        Some(msg)
                    }
                    _ => None,
                },
            )
            .collect(),
        false => vec![],
    }
}

fn create_reqwest_client() -> Result<blocking::Client, Box<dyn Error>> {
    let cert = reqwest::Certificate::from_pem(HUE_ROOT_CA.as_bytes())?;
    let client = blocking::ClientBuilder::new()
        .add_root_certificate(cert)
        .danger_accept_invalid_certs(true)
        .build()?;
    Ok(client)
}

fn get_request(
    bridge_ip: &BridgeIp,
    app_key: &AppKey,
    path: &str,
) -> Result<serde_json::Value, Box<dyn Error>> {
    let url = format!("https://{}{}", bridge_ip.0, path);
    println!("Requesting: {}", url);
    let response = create_reqwest_client()?
        .get(&url)
        .header("Accept", "application/json")
        .header("hue-application-key", String::from(app_key))
        .send()?;
    println!("Raw response: {:?}", response);
    if !response.status().is_success() {
        return Err(Box::new(HueError(
            format!(
                "Failed to send GET request to Hue Bridge: {}",
                &response.status()
            ),
            None,
        )));
    }
    let result = response.json::<serde_json::Value>()?;
    Ok(result)
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
    let body_str = serde_json::to_string(body)?;
    println!("Body: {:?}", body_str);
    let response = create_reqwest_client()?
        .post(&url)
        .header("Accept", "application/json")
        .body(body_str)
        .send()?;
    println!("Raw response: {:?}", response);
    if !response.status().is_success() {
        return Err(Box::new(HueError(
            format!(
                "Failed to send POST request to Hue Bridge: {}",
                &response.status()
            ),
            None,
        )));
    }
    let result = response.json::<serde_json::Value>()?;
    Ok(result)
}

/// Standard HUE device information.
#[derive(Debug, Clone, PartialEq)]
struct DeviceInfo {
    id: String,
    name: String,
    product_name: String,
    /// The service ID for a light device (for light devices only)
    light_id: Option<LightId>,
}

/// A Hue device on the bridge
#[derive(Debug, Clone, PartialEq)]
struct HueDevice(DeviceInfo);

fn list_devices(bridge_ip: &BridgeIp, api_key: &AppKey) -> Result<Vec<HueDevice>, HueError> {
    let response = get_request(&bridge_ip, &api_key, "/clip/v2/resource/device")
        .map_err(|e| HueError(e.to_string(), Some(e)))?;
    let parsed = parse_list_devices_response(&response)?;
    Ok(parsed)
}

/// Hue API representation of a device (some of the information)
#[derive(Deserialize, Debug)]
struct HueApiDeviceResponse {
    errors: Vec<HueApiErrorMessage>,
    data: Vec<HueApiDeviceData>,
}

/// Hue API representation of a device (some of the information)
#[derive(Deserialize, Debug)]
struct HueApiDeviceData {
    id: String,
    product_data: HueApiDeviceProductData,
    metadata: HueApiDeviceMetadata,
    services: Vec<HueApiDeviceService>,
}

/// Hue API representation of device product data (some of the information)
#[derive(Deserialize, Debug)]
struct HueApiDeviceProductData {
    model_id: String,
    product_name: String,
}
/// Hue API representation of device metadata (some of the information)
#[derive(Deserialize, Debug)]
struct HueApiDeviceMetadata {
    name: String,
}

/// Hue API representation of device service data (some of the information)
#[derive(Deserialize, Debug)]
struct HueApiDeviceService {
    rid: String,
    rtype: String,
}

fn parse_list_devices_response(json_response: &Value) -> Result<Vec<HueDevice>, HueError> {
    let parsed: HueApiDeviceResponse =
        serde_json::from_value::<HueApiDeviceResponse>(json_response.clone())
            .map_err(|e| HueError(e.to_string(), Some(Box::new(e))))?;
    match parsed.errors.is_empty() {
        true => Ok(parsed
            .data
            .into_iter()
            .map(|d| {
                HueDevice(DeviceInfo {
                    id: d.id,
                    name: d.metadata.name,
                    product_name: d.product_data.product_name,
                    light_id: d
                        .services
                        .iter()
                        .find(|s| s.rtype == "light")
                        .map(|s| LightId(s.rid.clone())),
                })
            })
            .collect()),
        false => Err(HueError(String::from("Response has errors"), None)),
    }
}

/// The body for the PUT /clip/v2/resource/light/{id} endpoint
/// See documentation at <https://developers.meethue.com/develop/hue-api-v2/core-concepts/#controlling-light>
#[derive(Serialize, Debug)]
struct LightControlRequestBody {
    on: LightOnOffState,
    #[serde(skip_serializing_if = "Option::is_none")]
    dimming: Option<LightDimmingState>,
}

#[derive(Serialize, Debug)]
struct LightOnOffState {
    on: bool,
}

#[derive(Serialize, Debug)]
struct LightDimmingState {
    brightness: f32,
}

/// A light ID, the service ID for a light device.
#[derive(Debug, Clone, PartialEq)]
struct LightId(String);
impl From<&LightId> for String {
    fn from(light_id: &LightId) -> Self {
        light_id.0.clone()
    }
}

fn control_light(
    bridge_ip: &BridgeIp,
    api_key: &AppKey,
    light_id: &LightId,
    on: bool,
    dimming_level: Option<u8>,
) -> Result<(), HueError> {
    let dimming = dimming_level.map(|level| {
        // Convert 0-100 scale to 0.0-100.0 brightness
        let brightness = f32::from(level.clamp(0, 100));
        LightDimmingState { brightness }
    });

    let body = LightControlRequestBody {
        on: LightOnOffState { on },
        dimming,
    };

    let path = format!("/clip/v2/resource/light/{}", String::from(light_id));
    put_request(&bridge_ip, &api_key, &path, &body)
        .map_err(|e| HueError(e.to_string(), Some(e)))?;
    Ok(())
}

/// Send a PUT request to the Hue Bridge.
fn put_request<T>(
    bridge_ip: &BridgeIp,
    app_key: &AppKey,
    path: &str,
    body: &T,
) -> Result<serde_json::Value, Box<dyn Error>>
where
    T: ?Sized + Serialize,
{
    let url = format!("https://{}{}", bridge_ip.0, path);
    println!("Requesting: {}", url);
    let body_str = serde_json::to_string(body)?;
    println!("Body: {:?}", body_str);
    let response = create_reqwest_client()?
        .put(&url)
        .header("Accept", "application/json")
        .header("hue-application-key", String::from(app_key))
        .body(body_str)
        .send()?;
    println!("Raw response: {:?}", response);
    if !response.status().is_success() {
        return Err(Box::new(HueError(
            format!(
                "Failed to send PUT request to Hue Bridge: {}",
                &response.status()
            ),
            None,
        )));
    }
    let result = response.json::<serde_json::Value>()?;
    Ok(result)
}

/// Find a light by ID or name.
/// First tries to match the input as a light ID.
/// If no match is found, queries the bridge for all devices and searches for a name match.
/// Returns the light ID if a single match is found.
fn find_light_by_id_or_name(
    bridge_ip: &BridgeIp,
    api_key: &AppKey,
    id_or_name: &str,
) -> Result<LightId, HueError> {
    // First, try to list all devices
    let devices = list_devices(bridge_ip, api_key)?;
    
    // Check if the input matches a light ID directly
    for HueDevice(device_info) in &devices {
        if let Some(light_id) = &device_info.light_id {
            if light_id.0 == id_or_name {
                return Ok(light_id.clone());
            }
        }
    }
    
    // If no direct ID match, search for name matches (case-insensitive substring)
    let name_query = id_or_name.to_lowercase();
    let mut matches = Vec::new();
    
    // Collect devices with matching names
    for HueDevice(device_info) in devices {
        if let Some(light_id) = device_info.light_id.clone() {
            if device_info.name.to_lowercase().contains(&name_query) {
                println!("Found matching light: {} ({})", device_info.name, light_id.0);
                matches.push((device_info, light_id));
            }
        }
    }
    
    match matches.len() {
        0 => Err(HueError(format!("No light found with ID or name matching '{}'", id_or_name), None)),
        1 => {
            let (device_info, light_id) = matches.remove(0);
            println!("Using light: {} ({})", device_info.name, light_id.0);
            Ok(light_id)
        },
        _ => {
            let match_info: Vec<String> = matches
                .iter()
                .map(|(info, _)| format!("{} ({})", info.name, info.id))
                .collect();
            Err(HueError(
                format!(
                    "Multiple lights found matching '{}'. Please be more specific or use the light ID directly: {}",
                    id_or_name,
                    match_info.join(", ")
                ),
                None,
            ))
        }
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let app_key_arg = Arg::new("key")
        .help("Application key for the Philips Hue API")
        .long("key")
        .value_name("KEY")
        .required(true);

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
        .subcommand(
            Command::new("list")
                .about("List all devices on the Hue Bridge.")
                .arg(app_key_arg.clone()),
        )
        .subcommand(
            Command::new("light")
                .about("Control a light")
                .arg(app_key_arg.clone())
                .arg(
                    Arg::new("id")
                        .help("The light device service ID or a part of the light name (case-insensitive substring search).")
                        .required(true)
                        .index(1)
                )
                .arg(
                    Arg::new("on")
                        .help("Turn the light on")
                        .long("on")
                        .action(clap::ArgAction::SetTrue)
                        .conflicts_with("off")
                )
                .arg(
                    Arg::new("off")
                        .help("Turn the light off")
                        .long("off")
                        .action(clap::ArgAction::SetTrue)
                        .conflicts_with("on")
                )
                .arg(
                    Arg::new("dim")
                        .help("Set the dimming level (0-100)")
                        .long("dim")
                        .value_name("LEVEL")
                        .value_parser(clap::value_parser!(u8).range(0..=100))
                )
        )
        .get_matches();

    if let Some(bridge_ip) = matches.get_one::<String>("bridge") {
        println!("Using Hue Bridge at: {}", bridge_ip);
        let bridge = BridgeIp(String::from(bridge_ip));
        if let Some(_sub_matches) = matches.subcommand_matches("create-key") {
            println!("Requesting creation of a new application key on the Hue Bridge. Make sure you have pressed the link button on the bridge!");
            let bridge_key = create_key(&bridge)?;
            println!("Key created: {:?}", bridge_key);
            Ok(())
        } else if let Some(list_matches) = matches.subcommand_matches("list") {
            let app_key = AppKey(String::from(
                list_matches
                    .get_one::<String>(app_key_arg.get_id().as_str())
                    .unwrap(),
            ));
            println!("Requesting list of devices on the Hue Bridge...");
            let devices = list_devices(&bridge, &app_key)?;
            println!(
                "{:36} | {:30} | {:20} | {:20}",
                "Device ID", "Name", "Product Name", "Light ID"
            );
            for HueDevice(di) in devices {
                println!(
                    "{:36} | {:30} | {:20} | {:20}",
                    di.id,
                    di.name,
                    di.product_name,
                    match di.light_id {
                        Some(light_id) => String::from(&light_id),
                        None => "".to_string(),
                    }
                );
            }
            Ok(())
        } else if let Some(light_matches) = matches.subcommand_matches("light") {
            let app_key = AppKey(String::from(
                light_matches
                    .get_one::<String>(app_key_arg.get_id().as_str())
                    .unwrap(),
            ));
            let id_or_name = light_matches.get_one::<String>("id").unwrap();

            let turn_on = match (light_matches.get_flag("on"), light_matches.get_flag("off")) {
                (true, false) => true,
                (false, true) => false,
                _ => {
                    return Err(Box::new(HueError(
                        String::from("Must specify either --on or --off"),
                        None,
                    )))
                }
            };

            // Get the dimming level if provided
            let dimming_level = light_matches.get_one::<u8>("dim").copied();

            println!(
                "Finding light with ID or name: {}",
                id_or_name
            );
            
            let light_id = find_light_by_id_or_name(&bridge, &app_key, id_or_name)?;
            
            // Update the message to include dimming information
            let state_message = match (turn_on, dimming_level) {
                (false, _) => "off".to_string(),
                (true, None) => "on".to_string(),
                (true, Some(level)) => format!("on with brightness {}%", level),
            };
            
            println!(
                "Setting light {} to {}",
                light_id.0,
                state_message
            );
            
            control_light(&bridge, &app_key, &light_id, turn_on, dimming_level)?;
            println!("Light state updated successfully");
            Ok(())
        } else {
            Err(Box::new(HueError(
                String::from("No subcommand provided. Please provide a subcommand."),
                None,
            )))
        }
    } else {
        Err(Box::new(HueError(
            String::from("No Hue Bridge IP address provided."),
            None,
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_api_response_errors_when_error_is_present() {
        let response_body = serde_json::json!(
        [
            {
                "error": {
                    "type": 101,
                    "address": "/",
                    "description": "link button not pressed"
                }
            }
        ]);
        let errors = parse_api_response_errors(&response_body);

        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].type_value, 101);
        assert_eq!(errors[0].address, "/");
        assert_eq!(errors[0].description, "link button not pressed");
        assert_eq!(
            errors[0],
            HueApiErrorMessage {
                type_value: 101,
                address: "/".to_string(),
                description: "link button not pressed".to_string(),
            }
        );
    }

    #[test]
    fn parse_api_response_errors_when_no_error_is_present() {
        let response_body = serde_json::json!(
        [
            {
                "success": {
                    "username": "1234567890"
                }
            }
        ]);
        let errors = parse_api_response_errors(&response_body);
        assert_eq!(errors.len(), 0);
    }

    #[test]
    fn parse_create_key_response_with_successful_operation() {
        let response_body = serde_json::json!(
        [
            {
                "success": {
                    "username": "1234567890"
                }
            }
        ]);
        let actual = parse_create_key_response(&response_body);
        assert_eq!(actual.is_ok(), true);
        assert_eq!(
            HueApiCreateKeySuccessDetails {
                user_name: "1234567890".to_string()
            },
            actual.unwrap()
        );
    }

    #[test]
    fn parse_list_devices_response_with_successful_operation_light_device() {
        let response_body = serde_json::json!(
            {"errors": [],
             "data": [
                {
                  "id": "94860050-1d86-4b79-8583-1be7dce05197",
                  "id_v1": "/lights/2",
                  "product_data": {
                    "model_id": "123455987123",
                    "manufacturer_name": "Signify Netherlands B.V.",
                    "product_name": "Space Light",
                    "product_archetype": "foo_bar",
                    "certified": true,
                    "software_version": "1.1.2",
                    "hardware_platform_type": "100b-118"
                  },
                  "metadata": {
                    "name": "Space light 1",
                    "archetype": "foo_bar"
                  },
                  "identify": {},
                  "services": [
                    {
                      "rid": "7d5545be-626a-4d63-a2f4-4347e43b50f6",
                      "rtype": "zigbee_connectivity"
                    },
                    {
                      "rid": "53ca6e61-5e40-4760-9e2e-6d2f48594901",
                      "rtype": "light"
                    },
                    {
                      "rid": "5dbe9888-a0b7-42d4-b002-9f15cd77e419",
                      "rtype": "entertainment"
                    },
                    {
                      "rid": "7c12995f-03bc-4b31-bb55-9da9e075dc0f",
                      "rtype": "taurus_7455"
                    },
                    {
                      "rid": "5b275c9c-dd12-45a8-9d36-716c43c1d3ed",
                      "rtype": "device_software_update"
                    }
                  ],
                  "type": "device"
                }
                ]
        }
        );

        let actual = parse_list_devices_response(&response_body);
        assert_eq!(actual.is_ok(), true);
        let ds = actual.unwrap();
        assert_eq!(ds.len(), 1);
        assert_eq!(
            ds[0],
            HueDevice(DeviceInfo {
                id: "94860050-1d86-4b79-8583-1be7dce05197".to_string(),
                name: "Space light 1".to_string(),
                product_name: "Space Light".to_string(),
                light_id: Some(LightId("53ca6e61-5e40-4760-9e2e-6d2f48594901".to_string())),
            })
        )
    }
}
