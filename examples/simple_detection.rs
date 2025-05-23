//! Simple detection example

use sigma_rs::{DynamicEvent, Event, Keyworder, Selector};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Create a sample event
    let event_data = serde_json::json!({
        "@timestamp": "2024-01-01T00:00:00Z",
        "event": {
            "action": "process_started",
            "category": "process"
        },
        "process": {
            "name": "powershell.exe",
            "command_line": "powershell.exe -EncodedCommand ...",
            "parent": {
                "name": "cmd.exe"
            }
        },
        "host": {
            "name": "WORKSTATION01"
        }
    });
    
    let event = DynamicEvent::new(event_data);
    
    // Test event selection
    let (process_name, found) = event.select("process.name");
    if found {
        tracing::error!("Process name: {:?}", process_name);
    }
    
    // Test nested selection
    let (parent_name, found) = event.select("process.parent.name");
    if found {
        tracing::error!("Parent process: {:?}", parent_name);
    }
    
    // Test keywords
    let (keywords, applicable) = event.keywords();
    if applicable {
        tracing::error!("Keywords: {:?}", keywords);
    }
    
    Ok(())
}
