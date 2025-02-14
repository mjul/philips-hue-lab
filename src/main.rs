use clap::Command;

fn main() {
    let _matches = Command::new("philips_hue_lab")
        .version("0.1.0")
        .about("Experimental CLI tools for Philips Hue ZigBee IoT devices.")
        .get_matches();
}
