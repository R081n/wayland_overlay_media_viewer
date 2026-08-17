use std::env;

use schemars::schema_for;
use wayland_overlay_media_viewer::spawner::ObjectMessage;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args[1] == "schema" {
        println!("{:?}", schema_for!(ObjectMessage));
        return;
    }
    let sender = wayland_overlay_media_viewer::startup();

    for line in std::io::stdin().lines() {
        let line = line.unwrap();

        match serde_json::from_str::<ObjectMessage>(&line) {
            Ok(msg) => {
                if let Err(e) = sender.send(msg) {
                    bevy::log::error!("Send Error: {e}");
                }
            }
            Err(e) => bevy::log::error!("Parse Error: {e}"),
        }
    }
}
