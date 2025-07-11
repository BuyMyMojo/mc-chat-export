use std::{
    fs::{self, File},
    io::Write,
    path::Path,
    process::exit,
};

use ab_glyph::{FontRef, PxScale};
use anyhow::{Error, Result};
use clap::Parser;
use csv::Writer;
use dialoguer::MultiSelect;
use image::{Rgb, RgbImage};
use imageproc::drawing::{draw_text_mut, text_size};
use once_cell::sync::Lazy;
use rayon::{
    iter::{IntoParallelIterator, ParallelIterator},
    str::ParallelString,
};
use regex::Regex;

/// Simple program to greet a person
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long, help = "Path to log file")]
    input: String,

    #[arg(
        short,
        long,
        help = "Format of output file",
        default_value_t,
        value_enum
    )]
    format: OutputFormat,

    #[arg(short, long, help = "Path to output  file")]
    output: String,
}

#[derive(clap::ValueEnum, Debug, Clone, Default)]
enum OutputFormat {
    Image,
    Csv,
    #[default]
    Txt,
}

fn main() -> Result<()> {
    let args = Args::parse();

    if !fs::exists(&args.input)? {
        println!("Please provide a valid file path!");
        exit(1)
    }

    let log_contents = fs::read_to_string(&args.input)?;

    let split_log = log_contents.par_split('\n');

    let filtered_log: Vec<&str> = split_log.filter(|x| is_possible_chat_msg(x)).collect();

    let extracted: Vec<String> = filtered_log
        .clone()
        .into_par_iter()
        .map(|x| {
            format!(
                "{} {}",
                extract_date_time(x).first().unwrap(),
                extract_message(x)
            )
        })
        .collect();

    let selection = MultiSelect::new()
        .with_prompt("What messages do you want to render?")
        .items(&extracted)
        .interact()?;

    match &args.format {
        OutputFormat::Image => save_image_file(&args.output, extracted, selection)?,
        OutputFormat::Csv => save_csv_file(&args.output, extracted, selection)?,
        OutputFormat::Txt => save_txt_file(&args.output, extracted, selection)?,
    }

    Ok(())
}

fn save_image_file(
    output: &String,
    extracted: Vec<String>,
    selection: Vec<usize>,
) -> Result<(), Error> {
    let out_path = Path::new(output);

    let font = FontRef::try_from_slice(include_bytes!("../font/Minecraft-Regular.ttf")).unwrap();

    let height = 40.0;
    let scale = PxScale {
        x: height * 2.0,
        y: height,
    };

    let mut selected: Vec<String> = vec![];

    if selection.is_empty() {
        selected = extracted;
    } else {
        for x in selection {
            selected.push(extracted[x].clone());
        }
    }

    let mut longest: String = selected.first().expect("selected list is empty?").clone();

    for msg in &selected {
        if msg.len() > longest.len() {
            longest = msg.clone();
        }
    }

    // let text = extracted[selection[0]].clone();
    let (w, h) = text_size(scale, &font, &longest);
    println!("Text size: {w}x{h}");

    let image_width = w + 8;
    let image_height = (h * selected.len() as u32) + (4 * selected.len() as u32) + 4;

    let mut image = RgbImage::new(image_width, image_height);

    for (current_line, msg) in (0_u32..).zip(selected.iter()) {
        draw_text_mut(
            &mut image,
            Rgb([254u8, 254u8, 254u8]),
            4,
            ((current_line * h) + (4 * current_line))
                .try_into()
                .unwrap(),
            scale,
            &font,
            msg,
        );
    }

    image.save(out_path).unwrap();
    Ok(())
}

fn save_txt_file(
    output: &String,
    extracted: Vec<String>,
    selection: Vec<usize>,
) -> Result<(), Error> {
    let mut out_file = File::create(output)?;

    if selection.is_empty() {
        for msg in extracted {
            out_file.write_all(msg.as_bytes())?;
            out_file.write_all(b"\n")?;
        }
    } else {
        for msg in selection {
            out_file.write_all(extracted[msg].as_bytes())?;
            out_file.write_all(b"\n")?;
        }
    }

    Ok(())
}

fn save_csv_file(
    output: &String,
    extracted: Vec<String>,
    selection: Vec<usize>,
) -> Result<(), Error> {
    let mut out_file = Writer::from_path(output)?;
    out_file.write_record(["date", "time", "msg"])?;

    let mut selected: Vec<String> = vec![];

    if selection.is_empty() {
        selected = extracted;
    } else {
        for x in selection {
            selected.push(extracted[x].clone());
        }
    }

    for msg in selected {
        let mut time: Vec<&str> = extract_date_time(&msg);

        // if there is only 1 entry in the vec then it has to be a client time so we can just add a pointless entry for the csv
        if time.len() == 1 {
            time.insert(0, "Null");
        }

        out_file.write_record([time[0], time[1], &extract_message(&msg)])?;
    }

    Ok(())
}

fn extract_message(msg: &str) -> String {
    /// follow up by extracting just the user and the message
    static EXTRACT_MSG: Lazy<Regex> = Lazy::new(|| Regex::new(r"<.*> .*").unwrap());

    EXTRACT_MSG
        .captures(msg)
        .expect("Unable to extract time")
        .get(0)
        .unwrap()
        .as_str()
        .to_string()
}

fn extract_date_time(msg: &str) -> Vec<&str> {
    static EXTRACT_TIME: Lazy<Regex> = Lazy::new(|| Regex::new(r"^\[.*?\]").unwrap());

    let time: Vec<&str> = EXTRACT_TIME
        .captures(msg)
        .expect("Unable to extract time")
        .get(0)
        .unwrap()
        .as_str()
        .strip_prefix("[")
        .expect("Unable to remove time prefix")
        .strip_suffix("]")
        .expect("Unable to remove time suffix")
        .split(' ')
        .collect();
    time
}

fn is_possible_chat_msg(input: &str) -> bool {
    /// first pass to find all possible lines that could have a chat message
    static SERVER_MSG_RE: Lazy<Regex> = Lazy::new(|| {
        Regex::new(
            r"\[.*\] \[Server thread\/INFO\] \[net.minecraft.server.MinecraftServer\/\]: <.*>*",
        )
        .unwrap()
    });
    static CLIENT_PRISM_MSG_RE: Lazy<Regex> = Lazy::new(|| {
        Regex::new(r"\[.*\] \[Render thread\/INFO\] \[minecraft\/ChatComponent\]: \[CHAT\] <.*>*")
            .unwrap()
    });
    static CLIENT_LOG_MSG_RE: Lazy<Regex> = Lazy::new(|| {
        Regex::new(r"\[.*\] \[Render thread\/INFO\] \[net.minecraft.client.gui.components.ChatComponent\/\]: \[CHAT\] <.*>*")
            .unwrap()
    });

    // This is here just in case I need to get a chat message from a strange place
    static _CLIENT_CATCHALL_MSG_RE: Lazy<Regex> =
        Lazy::new(|| Regex::new(r".*?: \[CHAT\] .*").unwrap());

    SERVER_MSG_RE.is_match(input)
        || CLIENT_PRISM_MSG_RE.is_match(input)
        || CLIENT_LOG_MSG_RE.is_match(input)
}

#[cfg(test)]
mod tests {
    use super::*;
    const TEST_SERVER_MSG: &str = "[05Jul2025 12:41:12.295] [Server thread/INFO] [net.minecraft.server.MinecraftServer/]: <🐈 Vftdan> :3";
    const TEST_CLIENT_LOG_MSG: &str = "[11Jul2025 20:30:20.286] [Render thread/INFO] [net.minecraft.client.gui.components.ChatComponent/]: [CHAT] <BuyMyMojo@MakeoutPoint> tehe";
    const TEST_PRISM_MSG: &str = "[16:53:50] [Render thread/INFO] [minecraft/ChatComponent]: [CHAT] <smol.systems-LeNooby_09> ;3";

    const RANDOM_LOG_LINES: [&str; 5] = [
        "[05Jul2025 13:08:11.886] [VoiceChatPacketProcessingThread/INFO] [voicechat/]: [voicechat] Player 399aedb6-a257-49d1-930b-af62fc328ae7 timed out",
        "[05Jul2025 13:09:19.890] [Server thread/INFO] [me.ichun.mods.serverpause.common.core.MinecraftServerMethods/]: Saving and pausing game...",
        "[05Jul2025 12:02:58.057] [Server thread/INFO] [net.minecraft.server.MinecraftServer/]: alto joined the game",
        "java.lang.NullPointerException: Cannot invoke \"net.minecraft.world.Container.getContainerSize()\" because the return value of \"net.neoforged.neoforge.items.wrapper.InvWrapper.getInv()\" is null",
        "[05Jul2025 10:31:36.112] [Server thread/INFO] [owo/]: Receiving client config",
    ];

    #[test]
    fn detect_server_chat_messages() {
        assert!(is_possible_chat_msg(TEST_SERVER_MSG));
    }

    #[test]
    fn detect_prism_chat_messages() {
        assert!(is_possible_chat_msg(TEST_PRISM_MSG));
    }

    #[test]
    fn detect_client_log_chat_messages() {
        assert!(is_possible_chat_msg(TEST_CLIENT_LOG_MSG));
    }

    #[test]
    fn extract_chat_message_server() {
        assert_eq!("<🐈 Vftdan> :3", &extract_message(TEST_SERVER_MSG));
    }
    #[test]
    fn extract_chat_message_client_log() {
        assert_eq!(
            "<BuyMyMojo@MakeoutPoint> tehe",
            &extract_message(TEST_CLIENT_LOG_MSG)
        );
    }
    #[test]
    fn extract_chat_message_prism_log() {
        assert_eq!(
            "<smol.systems-LeNooby_09> ;3",
            &extract_message(TEST_PRISM_MSG)
        );
    }

    #[test]
    fn extract_datetime_server_messages() {
        let msg_string = TEST_SERVER_MSG.to_string();
        let datetime: Vec<&str> = extract_date_time(&msg_string);
        println!("server datetime: {:?}", datetime);

        let correct_datetime: Vec<&str> = vec![&"05Jul2025", &"12:41:12.295"];
        assert_eq!(datetime, correct_datetime);
    }

    #[test]
    fn extract_datetime_client_messages() {
        let msg_string = TEST_CLIENT_LOG_MSG.to_string();
        let datetime: Vec<&str> = extract_date_time(&msg_string);
        println!("server datetime: {:?}", datetime);

        let correct_datetime: Vec<&str> = vec![&"11Jul2025", &"20:30:20.286"];
        assert_eq!(datetime, correct_datetime);
    }

    #[test]
    fn extract_datetime_prism_messages() {
        let msg_string = TEST_PRISM_MSG.to_string();
        let datetime: Vec<&str> = extract_date_time(&msg_string);
        println!("prism datetime: {:?}", datetime);

        let correct_datetime: Vec<&str> = vec![&"16:53:50"];
        assert_eq!(datetime, correct_datetime);
    }

    #[test]
    fn ignore_random_log_lines() {
        let mut is_msg: bool = false;

        for line in RANDOM_LOG_LINES {
            if is_possible_chat_msg(line) {
                is_msg = true;
            }
        }

        assert!(!is_msg);
    }
}
