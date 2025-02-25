# Philips Hue Lab

Experimental CLI tools for Philips Hue ZigBee IoT devices.

Features:
- discover bridges on the network
- enumerate devices on the network
- turn lights on and off
- query motion sensors
- turn power sockets on and off
- control light settings

## Design

In this lab we will build a Rust command-line tool.

- For parsing the arguments, we use the `clap` crate.

## Usage

You can build and run the application using `cargo run --`  and then
specifying the command line arguments, for example:

```powershell
cargo run -- --help
```

You can also build the application and then run the compiled binary.

```powershell
cargo build
.\target\debug\philips_hue_lab.exe --help
```

To use the application you must provide an API key. You can create an API key
using the `create-key` command. Press the Link button on the bridge shortly before 
running this command to authorize the key generation:

```powershell
.\target\debug\philips_hue_lab.exe create-key --bridge 192.168.1.2 
```

Once the key is created you can use it with the other commands to control the devices.

## License
MI License, see the [LICENSE](LICENSE) file.

