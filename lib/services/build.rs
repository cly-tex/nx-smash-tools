use std::collections::HashMap;
use std::fmt::Write;

use serde::Deserialize;

#[derive(Deserialize)]
enum HandleType {
    Move,
    Copy,
}

#[derive(Deserialize)]
struct Handle {
    #[serde(rename = "type")]
    ty: HandleType,
    name: String,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Header {
    send_pid: bool,
    handles: Vec<Handle>,
}

#[derive(Deserialize)]
struct PayloadItem {
    #[serde(rename = "type")]
    ty: String,
    name: String,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct CommandIO {
    header: Header,
    payload: Vec<PayloadItem>,
}

#[derive(Deserialize)]
struct Command {
    command_id: u32,
    input: CommandIO,
    output: CommandIO,
}

fn emit_command_io(
    out_code: &mut String,
    name: &str,
    is_in: bool,
    cmd_id: u32,
    command: &CommandIO,
) {
    let mut struct_size = 0;

    let _ = write!(
        out_code,
        "#[repr(C)] #[derive(Copy, Clone)] pub struct {name}{}{{",
        if is_in { "Request" } else { "Response" }
    );

    let _ = write!(out_code, "header0:crate::types::Header0Tag,");
    let _ = write!(out_code, "header1:crate::types::Header1Tag,");

    struct_size += 8;

    let has_special = command.header.send_pid || !command.header.handles.is_empty();
    if has_special {
        let _ = write!(out_code, "special:crate::types::SpecialTag,");
        struct_size += 4;
    }

    for handle in command
        .header
        .handles
        .iter()
        .filter(|handle| matches!(&handle.ty, HandleType::Copy))
    {
        let _ = write!(out_code, "{}:u32,", handle.name);
    }
    for handle in command
        .header
        .handles
        .iter()
        .filter(|handle| matches!(&handle.ty, HandleType::Move))
    {
        let _ = write!(out_code, "{}:u32,", handle.name);
    }
    struct_size += command.header.handles.len() * 4;

    let pre_padding_count = (struct_size % 0x10) / 4;
    let post_padding_count = 4 - pre_padding_count;
    let _ = write!(
        out_code,
        "pre_cmif_padding: [::core::mem::MaybeUninit<u32>;{pre_padding_count}],"
    );
    let _ = write!(
        out_code,
        "cmif_header:crate::types::Cmif{}Header,",
        if is_in { "In" } else { "Out" }
    );

    for payload_item in command.payload.iter() {
        let _ = write!(out_code, "{}:{},", payload_item.name, payload_item.ty);
    }

    let _ = write!(
        out_code,
        "post_cmif_padding: [::core::mem::MaybeUninit<u32>;{post_padding_count}],"
    );
    let _ = write!(out_code, "}}");

    // Implementation
    let _ = write!(
        out_code,
        "impl {name}{}{{",
        if is_in { "Request" } else { "Response" }
    );

    let _ = write!(
        out_code,
        "const HEADER0:crate::types::Header0Tag=crate::types::Header0Tag::new(0x4,0,0,0,0);"
    );
    let _ = write!(
        out_code,
        "const HEADER1:crate::types::Header1Tag=crate::types::Header1Tag::new((((::core::mem::offset_of!({name}{0}, post_cmif_padding) - ::core::mem::offset_of!({name}{0}, cmif_header)) / 4) + 4) as u32,0,0,{has_special});",
        if is_in { "Request" } else { "Response" }
    );
    if has_special {
        let _ = write!(
            out_code,
            "const SPECIAL:crate::types::SpecialTag=crate::types::SpecialTag::new({},{},{});",
            command.header.send_pid,
            command
                .header
                .handles
                .iter()
                .filter(|handle| matches!(&handle.ty, HandleType::Copy))
                .count(),
            command
                .header
                .handles
                .iter()
                .filter(|handle| matches!(&handle.ty, HandleType::Move))
                .count()
        );
    }

    let _ = write!(out_code, "pub const fn new(");
    for handle in command
        .header
        .handles
        .iter()
        .filter(|handle| matches!(&handle.ty, HandleType::Copy))
    {
        let _ = write!(out_code, "{}:u32,", handle.name);
    }
    for handle in command
        .header
        .handles
        .iter()
        .filter(|handle| matches!(&handle.ty, HandleType::Move))
    {
        let _ = write!(out_code, "{}:u32,", handle.name);
    }
    for item in command.payload.iter() {
        let _ = write!(out_code, "{}:{},", item.name, item.ty);
    }
    if !is_in {
        let _ = write!(out_code, "result:u32,");
    }
    let _ = write!(
        out_code,
        ")->Self{{Self{{header0:Self::HEADER0,header1:Self::HEADER1,"
    );
    if has_special {
        let _ = write!(out_code, "special:Self::SPECIAL,");
    }
    for handle in command
        .header
        .handles
        .iter()
        .filter(|handle| matches!(&handle.ty, HandleType::Copy))
    {
        let _ = write!(out_code, "{},", handle.name);
    }
    for handle in command
        .header
        .handles
        .iter()
        .filter(|handle| matches!(&handle.ty, HandleType::Move))
    {
        let _ = write!(out_code, "{},", handle.name);
    }
    let _ = write!(
        out_code,
        "pre_cmif_padding:[::core::mem::MaybeUninit::uninit();{pre_padding_count}],"
    );
    if is_in {
        let _ = write!(
            out_code,
            "cmif_header:crate::types::CmifInHeader::new({cmd_id}),"
        );
    } else {
        let _ = write!(
            out_code,
            "cmif_header:crate::types::CmifOutHeader::new(result,0),"
        );
    }
    for item in command.payload.iter() {
        let _ = write!(out_code, "{},", item.name);
    }
    let _ = write!(
        out_code,
        "post_cmif_padding:[::core::mem::MaybeUninit::uninit();{post_padding_count}],}}}}"
    );

    if !is_in {
        for handle in command.header.handles.iter() {
            let _ = write!(
                out_code,
                "pub const fn {0}(&self)->u32{{self.{0}}}",
                handle.name
            );
        }

        for item in command.payload.iter() {
            let _ = write!(
                out_code,
                "pub const fn {0}(&self)->{1}{{self.{0}}}",
                item.name, item.ty
            );
        }

        let _ = write!(
            out_code,
            "pub const fn result(&self)->u32{{self.cmif_header.result()}}"
        );
    }

    let _ = write!(out_code, "}}");
}

fn emit_command(out_code: &mut String, name: &str, command: &Command) {
    emit_command_io(out_code, name, true, command.command_id, &command.input);
    emit_command_io(out_code, name, false, command.command_id, &command.output);
}

fn main() {
    println!("cargo:rerun-if-changed=services.json");

    let modules: HashMap<String, HashMap<String, Command>> =
        serde_json::from_slice(&std::fs::read("services.json").unwrap()).unwrap();

    let mut emitted_rust = String::with_capacity(0x100);

    for (module_name, commands) in modules {
        let _ = write!(&mut emitted_rust, "pub mod {module_name}{{");
        for (name, command) in commands {
            emit_command(&mut emitted_rust, &name, &command);
        }
        let _ = writeln!(&mut emitted_rust, "}}");
    }

    let out_dir = std::env::var("OUT_DIR").unwrap();

    std::fs::write(format!("{out_dir}/generated_service_api.rs"), &emitted_rust).unwrap();

    assert!(
        std::process::Command::new("rustfmt")
            .arg(format!("{out_dir}/generated_service_api.rs"))
            .spawn()
            .unwrap()
            .wait()
            .unwrap()
            .success()
    );
}
