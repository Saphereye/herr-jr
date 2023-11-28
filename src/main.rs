use chrono::{Local, Timelike};
use dotenv::dotenv;
use lazy_static::lazy_static;
use reqwest::{Error, Url};
use serde_json::{from_str, to_string, Value};
use std::{
    collections::{HashMap, HashSet},
    time::Duration,
};
use teloxide::{
    prelude::*,
    types::{InputFile, ParseMode},
    utils::command::BotCommands,
};
use tokio::{sync::Mutex, time::sleep};

lazy_static! {
    static ref TODO_LIST: Mutex<HashMap<ChatId, Vec<String>>> = Mutex::new(HashMap::new());
    static ref USERS_LIST: Mutex<HashSet<ChatId>> = Mutex::new(HashSet::new());
}

#[tokio::main]
async fn main() {
    pretty_env_logger::init();
    log::info!("Starting command bot...");

    let bot = match dotenv().ok() {
        Some(_) => match std::env::var("TELOXIDE_TOKEN") {
            Ok(value) => {
                log::info!("TELOXIDE_TOKEN found in .env file");
                Bot::new(value)
            }
            Err(e) => {
                log::error!("Failed to read TELOXIDE_TOKEN from .env file: {}", e);
                log::info!("Trying from environment");
                Bot::from_env()
            }
        },
        None => {
            log::info!("TELOXIDE_TOKEN not found in .env file, trying from environment");
            Bot::from_env()
        }
    };

    log::info!("Reading todo.txt...");
    match std::fs::read_to_string("todo.json") {
        Ok(content) => {
            let tasks: HashMap<ChatId, Vec<String>> = from_str(&content).unwrap();
            *TODO_LIST.lock().await = tasks;
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            log::info!("todo.json not found");
        }
        Err(err) => {
            log::error!("Failed to read todo.json: {}", err);
        }
    }

    log::info!("Reading users.txt...");
    match std::fs::read_to_string("users.txt") {
        Ok(content) => {
            let users: Vec<ChatId> = content
                .lines()
                .map(|line| ChatId(line.parse::<i64>().unwrap()))
                .collect();
            *USERS_LIST.lock().await = users.into_iter().collect();
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            log::info!("users.txt not found");
        }
        Err(err) => {
            log::error!("Failed to read users.txt: {}", err);
        }
    }

    send_to_all(
        &bot,
        "Bot started successfully. Use /help to see available commands.",
    )
    .await;

    // log::info!("Sending greeting messages...");
    // for user in USERS_LIST.lock().await.iter() {
    //     let resp = reqwest::get("https://wttr.in/Hyderabad?format=%l:+%c+%t+%p+%m").await.unwrap();
    //     let content = resp.text().await.unwrap();
    //     bot.send_message(user.clone(), format!("Hi!\n\nToday's weather in {}\n\nYour todo list is: \n-{}", content, TODO_LIST.lock().await.join("\n-"))).await.unwrap();
    // }

    let bot_copy = bot.clone();

    tokio::spawn(async move {
        loop {
            let now = Local::now();
            let next_time = (now.date() + chrono::Duration::days(1)).and_hms(8, 0, 0); // Next day at 8 AM
            let duration_until_next_time = (next_time - now)
                .to_std()
                .unwrap_or_else(|_| std::time::Duration::from_secs(0));

            sleep(duration_until_next_time).await;
            log::info!("Sending greeting messages...");
            let resp = reqwest::get("https://wttr.in/Hyderabad?format=%l:+%c+%t+%p+%m")
                .await
                .unwrap();
            let content = resp.text().await.unwrap();
            send_to_all(
                &bot,
                format!("Good Morning!\n\nToday's weather in {}", content,).as_str(),
            )
            .await;
        }
    });

    Command::repl(bot_copy.clone(), answer).await;
    send_to_all(&bot_copy, "The bot is shutting down.").await;
    log::info!("Stopping bot...");

    log::info!("Writing todo.txt...");
    let todo_list = TODO_LIST.lock().await;
    let json = to_string(&*todo_list).unwrap();
    std::fs::write("todo.json", json).unwrap();

    log::info!("Writing users list...");
    let users_list = USERS_LIST.lock().await.clone();
    let content = users_list
        .iter()
        .map(|user| user.to_string())
        .collect::<Vec<String>>()
        .join("\n");
    std::fs::write("users.txt", content).expect("Unable to write file");
}

async fn send_to_all(bot: &Bot, msg: &str) {
    for user in USERS_LIST.lock().await.iter() {
        bot.send_message(*user, msg).await.unwrap();
    }
}

#[derive(BotCommands, Clone, Debug)]
#[command(
    rename_rule = "lowercase",
    description = "These commands are supported:"
)]
enum Command {
    #[command(description = "display this text.")]
    Help,
    #[command(description = "get a random cat image")]
    Cat,
    #[command(description = "get definition of the word")]
    Define(String),
    #[command(description = "get useless facts")]
    Useless,
    #[command(description = "get raw source of github file")]
    Raw(String),
    #[command(description = "returns current weather status")]
    Weather,
    #[command(description = "roll a dice")]
    Dice,
    #[command(description = "toss a coin")]
    Coin,
    #[command(description = "add to todo list")]
    Todo(String),
    #[command(description = "show contents of todo list")]
    List,
}

async fn answer(bot: Bot, msg: Message, cmd: Command) -> ResponseResult<()> {
    log::info!("Got command {:?}", cmd);

    if !USERS_LIST.lock().await.contains(&msg.chat.id) {
        USERS_LIST.lock().await.insert(msg.chat.id);
        bot.send_message(
            msg.chat.id,
            format!(
                "Hi {}!",
                (msg.from().expect("Invalid user").username.clone()).expect("Invalid string")
            ),
        )
        .await?;
    }

    match cmd {
        Command::Help => {
            bot.send_message(
                msg.chat.id,
                format!(
                    "Hi {} !\n\nThis Bot was made by <b>Herr Das</b>\n\n{}",
                    msg.from().expect("No user found").first_name.clone(),
                    Command::descriptions()
                ),
            )
            .parse_mode(ParseMode::Html)
            .await?
        }
        Command::Cat => {
            if let Ok(url) = get_cat_image().await {
                bot.send_photo(
                    msg.chat.id,
                    InputFile::url(Url::parse(&url).expect("Incorrect url")),
                )
                .await?
            } else {
                bot.send_message(msg.chat.id, "Failed to fetch cat image.")
                    .await?
            }
        }
        Command::Define(word) => {
            let url = format!("https://api.dictionaryapi.dev/api/v2/entries/en/{}", word);
            let resp = reqwest::get(&url).await?;
            let json: serde_json::Value = resp.json().await?;
            let mut content = String::new();
            for meaning in json[0]["meanings"].as_array().unwrap() {
                content.push_str(&format!(
                    "{}\n",
                    meaning["definitions"][0]["definition"].as_str().unwrap()
                ));
            }
            bot.send_message(msg.chat.id, content).await?
        }
        Command::Useless => {
            let resp = reqwest::get("https://uselessfacts.jsph.pl/random.json?language=en").await?;
            let json: serde_json::Value = resp.json().await?;
            bot.send_message(msg.chat.id, json["text"].as_str().unwrap())
                .await?
        }
        Command::Raw(file) => {
            let content = file
                .replace("github.com", "raw.githubusercontent.com")
                .replace("/blob/", "/");
            bot.send_message(msg.chat.id, content).await?
        }
        Command::Weather => {
            let resp = reqwest::get("https://wttr.in/Hyderabad?format=%l:+%c+%t+%p+%m").await?;
            let content = resp.text().await?;
            bot.send_message(msg.chat.id, content).await?
        }
        Command::Dice => bot.send_dice(msg.chat.id).await?,
        Command::Coin => bot.send_message(msg.chat.id, "🪙").await?,
        Command::Todo(task) => {
            log::info!("Adding '{}' to todo list", task);
            let mut todo_list = TODO_LIST.lock().await;
            let user_todo_list = todo_list.entry(msg.chat.id).or_insert_with(Vec::new);
            user_todo_list.push(task.clone());
            bot.send_message(msg.chat.id, format!("Added <u>{}</u> to todo list", task))
                .parse_mode(ParseMode::Html)
                .await?
        }
        Command::List => {
            let mut content = "<u>Todo list:</u>\n".to_string();
            for (i, task) in (TODO_LIST.lock().await)[&msg.chat.id].iter().enumerate() {
                content.push_str(&format!("{}. {}\n", i + 1, task));
            }
            bot.send_message(msg.chat.id, content)
                .parse_mode(ParseMode::Html)
                .await?
        }
    };

    Ok(())
}

async fn get_cat_image() -> Result<String, Error> {
    let resp = reqwest::get("https://api.thecatapi.com/v1/images/search").await?;
    let images: Vec<serde_json::Value> = resp.json().await?;
    Ok(images[0]["url"].as_str().unwrap().to_string())
}
