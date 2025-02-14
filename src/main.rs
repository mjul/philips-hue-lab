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
        .get_matches();

    // You can access the provided bridge IP address using:
    if let Some(bridge_ip) = matches.get_one::<String>("bridge") {
        println!("Using Hue Bridge at: {}", bridge_ip);
    }
}
