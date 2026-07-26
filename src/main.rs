use wayland_overlay_media_viewer::spawner::ObjectMessage;

fn main() {
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
