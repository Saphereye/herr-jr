use lazy_static::lazy_static;
use reqwest::{Error, Url};
use teloxide::{
    prelude::*,
    types::{InputFile, ParseMode},
    utils::command::BotCommands,
};
use tokio::sync::Mutex;

lazy_static! {
    static ref TODO_LIST: Mutex<Vec<String>> = Mutex::new(Vec::new());
}

#[tokio::main]
async fn main() {
    pretty_env_logger::init();
    log::info!("Starting command bot...");

    let bot = Bot::from_env();

    log::info!("Reading todo.txt...");
    match std::fs::read_to_string("todo.txt") {
        Ok(content) => {
            let tasks: Vec<String> = content.lines().map(|line| line.to_string()).collect();
            *TODO_LIST.lock().await = tasks;
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            log::info!("todo.txt not found");
        }
        Err(err) => {
            log::error!("Failed to read todo.txt: {}", err);
        }
    }

    Command::repl(bot, answer).await;
    log::info!("Stopping bot...");

    log::info!("Writing todo.txt...");
    let todo_list = TODO_LIST.lock().await.clone();
    let content = todo_list.join("\n");
    std::fs::write("todo.txt", content).expect("Unable to write file");

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
    match cmd {
        Command::Help => {
            bot.send_message(
                msg.chat.id,
                format!("Made by <b>Herr Das</b>\n\n{}", Command::descriptions()),
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
            let resp = reqwest::get("https://wttr.in/?format=%l:+%c+%t+%p+%m\n").await?;
            let content = resp.text().await?;
            bot.send_message(msg.chat.id, content).await?
        }
        Command::Dice => bot.send_dice(msg.chat.id).await?,
        Command::Coin => bot.send_message(msg.chat.id, "🪙").await?,
        Command::Todo(task) => {
            log::info!("Adding '{}' to todo list", task);
            TODO_LIST.lock().await.push(task.clone());
            bot.send_message(msg.chat.id, format!("Added <u>{}</u> to todo list", task))
                .parse_mode(ParseMode::Html)
                .await?
        }
        Command::List => {
            let mut content = "<u>Todo list:</u>\n".to_string();
            for (i, task) in TODO_LIST.lock().await.iter().enumerate() {
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
