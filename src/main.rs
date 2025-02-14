use clap::{Arg, Command};

fn main() {
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
        .arg(
            Arg::new("create-key")
                .long("create-key")
                .help("Ask the Hue Bridge to generate an application key (press the link button on the bridge to authorize this operation).")
                .action(clap::ArgAction::SetTrue),
        )
        .get_matches();

    if let Some(bridge_ip) = matches.get_one::<String>("bridge") {
        println!("Using Hue Bridge at: {}", bridge_ip);
    } else {
        println!("No Hue Bridge IP address provided.");
    }

    if *matches.get_one::<bool>("create-key").unwrap_or(&false) {
        println!("Requesting creation of a new application key on the Hue Bridge. Make sure you have pressed the link button on the bridge!");
        // TODO
    }
}