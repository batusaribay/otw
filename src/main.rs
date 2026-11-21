mod cache;
mod client;
mod games;
mod html;
mod style;

use std::collections::VecDeque;
use std::io::{self, Write};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};

use anyhow::{Context, Result, bail};
use clap::{CommandFactory, FromArgMatches, Parser, Subcommand};
use serde::Serialize;

use crate::cache::{Cache, human_age, human_bytes};
use crate::client::{Client, PAGE_TTL};
use crate::games::{Category, Game};
use crate::html::{Link, RenderOpts, Section};
use crate::style::{ColorMode, St, Style, visible_width};

#[derive(Parser)]
#[command(name = "otw", version, about = "A Rust CLI for OverTheWire games")]
struct Cli {
    /// Use only locally cached pages; never touch the network
    #[arg(long, global = true)]
    offline: bool,

    /// Re-download pages instead of using the cache
    #[arg(long, global = true)]
    refresh: bool,

    /// Show link URLs after link text
    #[arg(long, global = true)]
    links: bool,

    /// Wrap output at COLS columns (0 disables wrapping)
    #[arg(short = 'w', long, global = true, value_name = "COLS")]
    width: Option<usize>,

    /// Color output: auto, always, or never
    #[arg(
        long,
        global = true,
        value_enum,
        default_value = "auto",
        value_name = "WHEN"
    )]
    color: ColorMode,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// List every game and its server
    #[command(alias = "list")]
    Games {
        /// Print machine-readable JSON
        #[arg(long)]
        json: bool,
    },

    /// Show a game's overview and beginner notes
    #[command(alias = "show")]
    Info {
        /// Game name, e.g. bandit
        game: String,
        /// Print machine-readable JSON
        #[arg(long)]
        json: bool,
    },

    /// List the levels of a game
    #[command(alias = "ls")]
    Levels {
        /// Game name, e.g. bandit
        game: String,
        /// Print machine-readable JSON
        #[arg(long)]
        json: bool,
    },

    /// Show the task for the level you are logged in as
    #[command(alias = "l")]
    Level {
        /// Game name, e.g. bandit
        game: String,
        /// Level you are logged in as (default: 0)
        level: Option<String>,
        /// Print machine-readable JSON
        #[arg(long)]
        json: bool,
    },

    /// Print (or run) the ssh command for a level
    #[command(alias = "connect")]
    Ssh {
        /// Game name, e.g. bandit
        game: String,
        /// Level to log in as (default: 0)
        level: Option<String>,
        /// Actually start ssh (web games open a browser)
        #[arg(long)]
        run: bool,
    },

    /// Search the notes of every level
    #[command(alias = "find")]
    Search {
        /// Text to look for
        query: String,
        /// Only search this game
        #[arg(short, long, value_name = "GAME")]
        game: Option<String>,
        /// Parallel downloads
        #[arg(short, long, default_value_t = 4, value_name = "N")]
        jobs: usize,
        /// Stop after N matches (0 = unlimited)
        #[arg(short = 'n', long, default_value_t = 40, value_name = "N")]
        limit: usize,
        /// Print machine-readable JSON
        #[arg(long)]
        json: bool,
    },

    /// Show the suggested order for playing the games
    Start,

    /// Show the game rules
    Rules {
        /// Print machine-readable JSON
        #[arg(long)]
        json: bool,
    },

    /// Render any page from overthewire.org
    #[command(alias = "cat")]
    Page {
        /// Path such as /rules/ (or a full overthewire.org URL)
        path: String,
        /// Print machine-readable JSON
        #[arg(long)]
        json: bool,
    },

    /// Inspect or manage the local page cache
    Cache {
        #[command(subcommand)]
        command: CacheCommand,
    },
}

#[derive(Subcommand)]
enum CacheCommand {
    /// Show cache location, size and age
    Status {
        /// Print machine-readable JSON
        #[arg(long)]
        json: bool,
    },
    /// Delete every cached page
    Clear,
    /// Download pages so they are available offline
    Fetch {
        /// Games to cache, e.g. bandit natas (default: all games with levels)
        games: Vec<String>,
        /// Cache every game with levels
        #[arg(long)]
        all: bool,
        /// Parallel downloads
        #[arg(short, long, default_value_t = 4, value_name = "N")]
        jobs: usize,
    },
}

struct Cfg {
    style: Style,
    width: usize,
    links: bool,
    offline: bool,
    refresh: bool,
}

impl Cfg {
    fn render_opts(&self) -> RenderOpts {
        RenderOpts {
            width: self.width,
            style: self.style,
            links: self.links,
        }
    }
}

fn main() {
    let matches = Cli::command().color(clap::ColorChoice::Never).get_matches();
    let cli = Cli::from_arg_matches(&matches).unwrap_or_else(|error| error.exit());

    let Some(command) = cli.command else {
        let mut command = Cli::command().color(clap::ColorChoice::Never);
        if let Err(error) = command.print_help()
            && error.kind() != io::ErrorKind::BrokenPipe
        {
            eprintln!("error: could not print help: {error}");
            std::process::exit(1);
        }
        return;
    };

    let is_tty = style::stdout_is_tty();
    let width = match cli.width {
        Some(width) => width,
        None if is_tty => style::terminal_width().clamp(40, 100),
        None => 0,
    };
    let cfg = Cfg {
        style: Style::new(cli.color),
        width,
        links: cli.links,
        offline: cli.offline,
        refresh: cli.refresh,
    };
    if let Err(error) = run(command, &cfg) {
        eprintln!("error: {error:#}");
        std::process::exit(1);
    }
}

fn run(command: Command, cfg: &Cfg) -> Result<()> {
    match command {
        Command::Games { json } => {
            let client = Client::new(cfg.offline, cfg.refresh)?;
            let games = games::load(&client)?;
            cmd_games(&games, cfg, json)
        }
        Command::Info { game, json } => {
            let client = Client::new(cfg.offline, cfg.refresh)?;
            let games = games::load(&client)?;
            cmd_info(&client, &games, &game, cfg, json)
        }
        Command::Levels { game, json } => {
            let client = Client::new(cfg.offline, cfg.refresh)?;
            let games = games::load(&client)?;
            cmd_levels(&games, &game, cfg, json)
        }
        Command::Level { game, level, json } => {
            let client = Client::new(cfg.offline, cfg.refresh)?;
            let games = games::load(&client)?;
            cmd_level(&client, &games, &game, level.as_deref(), cfg, json)
        }
        Command::Ssh { game, level, run } => {
            let client = Client::new(cfg.offline, cfg.refresh)?;
            let games = games::load(&client)?;
            cmd_ssh(&games, &game, level.as_deref(), run)
        }
        Command::Search {
            query,
            game,
            jobs,
            limit,
            json,
        } => {
            let client = Client::new(cfg.offline, cfg.refresh)?;
            let games = games::load(&client)?;
            cmd_search(
                &client,
                &games,
                &query,
                SearchArgs {
                    only_game: game.as_deref(),
                    jobs,
                    limit,
                    json,
                },
                cfg,
            )
        }
        Command::Start => {
            cmd_start(cfg);
            Ok(())
        }
        Command::Rules { json } => {
            let client = Client::new(cfg.offline, cfg.refresh)?;
            let raw = client.get("rules/", PAGE_TTL)?;
            let raw = expand_includes(&client, &raw, "rules/");
            let body = html::render_page(&raw, &cfg.render_opts());
            if json {
                let page = html::parse_page(&raw);
                print_json(&PageJson {
                    path: "rules/".to_string(),
                    url: format!("{}/rules/", client::BASE),
                    title: page.title,
                    sections: page.sections,
                    links: page.links,
                    text: page.text,
                })
            } else {
                if body.is_empty() {
                    bail!("the rules page has no readable content");
                }
                emit(&body);
                Ok(())
            }
        }
        Command::Page { path, json } => {
            let client = Client::new(cfg.offline, cfg.refresh)?;
            cmd_page(&client, &path, cfg, json)
        }
        Command::Cache { command } => cmd_cache(command, cfg),
    }
}

fn emit(text: &str) {
    let mut stdout = io::stdout().lock();
    if let Err(error) = stdout
        .write_all(text.as_bytes())
        .and_then(|_| stdout.flush())
    {
        if error.kind() == io::ErrorKind::BrokenPipe {
            std::process::exit(0);
        }
        eprintln!("error: could not write output: {error}");
        std::process::exit(1);
    }
}

fn print_json<T: Serialize>(value: &T) -> Result<()> {
    let text = serde_json::to_string_pretty(value)?;
    emit(&format!("{text}\n"));
    Ok(())
}

fn footer_row(cfg: &Cfg, label: &str, value: &str) -> String {
    format!(
        "{} {}\n",
        cfg.style.paint(St::Dim, &format!("{label:<7}")),
        value
    )
}

fn section_heading(cfg: &Cfg, text: &str) -> String {
    format!("{}\n\n", cfg.style.paint(St::Bold, text))
}

#[derive(Serialize)]
struct GameJson {
    key: String,
    name: String,
    category: Category,
    description: &'static str,
    last_level: Option<u32>,
    last_page: Option<u32>,
    special_numbering: bool,
    host: Option<String>,
    port: Option<u16>,
    url: String,
    ssh: Option<String>,
}

impl GameJson {
    fn new(game: &Game) -> Self {
        Self {
            key: game.key.clone(),
            name: game.name.clone(),
            category: game.category,
            description: game.blurb,
            last_level: game.level_range().ok().map(|range| *range.end()),
            last_page: game.last,
            special_numbering: game.special,
            host: game.host.clone(),
            port: game.port,
            url: game.info_url(),
            ssh: game.ssh_command(0),
        }
    }
}

#[derive(Serialize)]
struct GameListJson {
    games: Vec<GameJson>,
}

fn cmd_games(games: &[Game], cfg: &Cfg, json: bool) -> Result<()> {
    if json {
        return print_json(&GameListJson {
            games: games.iter().map(GameJson::new).collect(),
        });
    }

    let name_width = games
        .iter()
        .map(|game| visible_width(&game.name))
        .max()
        .unwrap_or(10)
        .max(8)
        + 2;
    let blurb_width = games
        .iter()
        .map(|game| game.blurb.len())
        .max()
        .unwrap_or(16)
        + 2;

    let mut out = String::new();
    for category in [Category::Online, Category::Offline, Category::Released] {
        let group: Vec<&Game> = games
            .iter()
            .filter(|game| game.category == category)
            .collect();
        if group.is_empty() {
            continue;
        }
        out.push_str(&cfg.style.paint(St::Bold, category.title()));
        out.push('\n');
        for game in group {
            out.push_str(&format!(
                "  {:<name_width$} {:>7}   {:<blurb_width$} {}\n",
                game.name,
                game.levels_label(),
                game.blurb,
                cfg.style.paint(St::Dim, &game.where_label())
            ));
        }
        out.push('\n');
    }

    while out.ends_with("\n\n") {
        out.pop();
    }
    emit(&out);
    Ok(())
}

#[derive(Serialize)]
struct InfoJson {
    key: String,
    name: String,
    category: Category,
    description: &'static str,
    host: Option<String>,
    port: Option<u16>,
    last_level: Option<u32>,
    last_page: Option<u32>,
    special_numbering: bool,
    url: String,
    title: String,
    sections: Vec<Section>,
    links: Vec<Link>,
    text: String,
}

fn cmd_info(client: &Client, games: &[Game], name: &str, cfg: &Cfg, json: bool) -> Result<()> {
    let game = Game::find(games, name)?;
    let raw = client
        .get(&game.info_path(), PAGE_TTL)
        .with_context(|| format!("loading the {} overview", game.name))?;
    let raw = expand_includes(client, &raw, &game.info_path());

    if json {
        let page = html::parse_page(&raw);
        return print_json(&InfoJson {
            key: game.key.clone(),
            name: game.name.clone(),
            category: game.category,
            description: game.blurb,
            host: game.host.clone(),
            port: game.port,
            last_level: game.level_range().ok().map(|range| *range.end()),
            last_page: game.last,
            special_numbering: game.special,
            url: game.info_url(),
            title: page.title,
            sections: page.sections,
            links: page.links,
            text: page.text,
        });
    }

    let body = html::render_page(&raw, &cfg.render_opts());
    if body.is_empty() {
        bail!("the {} overview has no readable content", game.name);
    }

    let mut out = body;
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out.push('\n');

    if let (Some(host), Some(port)) = (&game.host, game.port) {
        out.push_str(&footer_row(cfg, "SSH", &format!("{host} port {port}")));
    } else if let Some(url) = game.web_url(0) {
        out.push_str(&footer_row(cfg, "Web", &game.where_label()));
        out.push_str(&footer_row(cfg, "Start", &url));
    } else {
        out.push_str(&footer_row(
            cfg,
            "Status",
            "released offline game · download only",
        ));
    }

    if game.last.is_some() {
        out.push_str(&footer_row(cfg, "Levels", &game.levels_label()));
    }

    emit(&out);
    Ok(())
}

#[derive(Serialize)]
struct LevelItem {
    level: u32,
    page: u32,
    title: String,
    url: String,
}

#[derive(Serialize)]
struct LevelsJson {
    key: String,
    name: String,
    last_level: u32,
    levels: Vec<LevelItem>,
}

fn cmd_levels(games: &[Game], name: &str, cfg: &Cfg, json: bool) -> Result<()> {
    let game = Game::find(games, name)?;
    let range = game.level_range()?;
    let last = *range.end();

    if json {
        let levels = (0..=last)
            .map(|level| {
                let page = game.task_page(level).expect("level range is valid");
                LevelItem {
                    level,
                    page,
                    title: game.level_title(page),
                    url: game.level_url(page),
                }
            })
            .collect();
        return print_json(&LevelsJson {
            key: game.key.clone(),
            name: game.name.clone(),
            last_level: last,
            levels,
        });
    }

    let width = last.to_string().len();
    let mut out = String::new();
    out.push_str(&cfg.style.paint(St::Accent, &game.name));
    out.push_str(
        &cfg.style
            .paint(St::Dim, &format!(" · {} levels (0-{last})\n\n", last + 1)),
    );
    for level in 0..=last {
        let page = game.task_page(level).expect("level range is valid");
        out.push_str(&format!(
            "  {}  {}\n",
            cfg.style.paint(St::Dim, &format!("{level:>width$}")),
            game.level_title(page)
        ));
    }
    emit(&out);
    Ok(())
}

#[derive(Serialize)]
struct LevelJson {
    game: String,
    name: String,
    level: u32,
    page: u32,
    title: String,
    url: String,
    read_as: Option<String>,
    ssh: Option<String>,
    web: Option<String>,
    start: Option<StartJson>,
    sections: Vec<Section>,
    links: Vec<Link>,
    text: String,
}

#[derive(Serialize)]
struct StartJson {
    title: String,
    url: String,
    sections: Vec<Section>,
    links: Vec<Link>,
    text: String,
}

struct StartPage {
    title: String,
    url: String,
    body: String,
    page: html::Page,
}

fn cmd_level(
    client: &Client,
    games: &[Game],
    name: &str,
    level: Option<&str>,
    cfg: &Cfg,
    json: bool,
) -> Result<()> {
    let game = Game::find(games, name)?;
    let (level_number, page_number) = match level {
        Some(level) => {
            let level_number = games::parse_task_level(game, level)?;
            (level_number, game.task_page(level_number)?)
        }
        None => (0, game.task_page(0)?),
    };
    show_level(client, game, level_number, page_number, cfg, json)
}

fn show_level(
    client: &Client,
    game: &Game,
    level_number: u32,
    page_number: u32,
    cfg: &Cfg,
    json: bool,
) -> Result<()> {
    let raw = client
        .get(&game.level_path(page_number), PAGE_TTL)
        .with_context(|| format!("loading {} level {page_number}", game.name))?;
    let raw = expand_includes(client, &raw, &game.level_path(page_number));

    let title = game.level_title(page_number);
    let login_level = game.login_level(page_number);

    // Level 0 of a normal shell game starts with the login page (page 0) and
    // then the first task (page 1). Show both so one command starts the game.
    let include_start = level_number == 0 && page_number > 0;
    let start = if include_start {
        let start_path = game.level_path(0);
        let start_raw = client
            .get(&start_path, PAGE_TTL)
            .with_context(|| format!("loading {} starting page", game.name))?;
        let start_raw = expand_includes(client, &start_raw, &start_path);
        Some(StartPage {
            title: game.level_title(0),
            url: game.level_url(0),
            body: html::render_page(&start_raw, &cfg.render_opts()),
            page: html::parse_page(&start_raw),
        })
    } else {
        None
    };

    if json {
        let page = html::parse_page(&raw);
        return print_json(&LevelJson {
            game: game.key.clone(),
            name: game.name.clone(),
            level: level_number,
            page: page_number,
            title,
            url: game.level_url(page_number),
            read_as: game.read_as(page_number),
            ssh: game.ssh_command(login_level),
            web: game.web_url(page_number),
            start: start.as_ref().map(|start| StartJson {
                title: start.title.clone(),
                url: start.url.clone(),
                sections: start.page.sections.clone(),
                links: start.page.links.clone(),
                text: start.page.text.clone(),
            }),
            sections: page.sections,
            links: page.links,
            text: page.text,
        });
    }

    let body = html::render_page(&raw, &cfg.render_opts());
    if body.is_empty() {
        bail!("{} level {page_number} has no readable content", game.name);
    }

    let read_as = game
        .read_as(page_number)
        .map(|user| format!("Read as {user}"))
        .unwrap_or_else(|| "Starting point".to_string());

    let mut out = String::new();
    out.push_str(&cfg.style.paint(St::Accent, &game.name));
    out.push_str(&cfg.style.paint(St::Dim, " · "));
    if include_start {
        out.push_str(&cfg.style.paint(St::Bold, &game.level_title(0)));
    } else {
        out.push_str(&cfg.style.paint(St::Bold, &title));
    }
    out.push('\n');
    out.push_str(&cfg.style.paint(St::Dim, &read_as));
    out.push_str("\n\n");
    if let Some(start) = &start {
        out.push_str(&section_heading(cfg, "Start here"));
        out.push_str(&start.body);
        if !start.body.ends_with('\n') {
            out.push('\n');
        }
        out.push('\n');
        out.push_str(&section_heading(cfg, "First task"));
    }
    out.push_str(&body);
    emit(&out);
    Ok(())
}

fn cmd_ssh(games: &[Game], name: &str, level: Option<&str>, run: bool) -> Result<()> {
    let game = Game::find(games, name)?;
    let login_level = match level {
        Some(level) => games::parse_level(game, level)?,
        None => 0,
    };

    if let Some(argv) = game.ssh_argv(login_level) {
        if run {
            let status = std::process::Command::new("ssh")
                .args(&argv)
                .status()
                .context("could not start ssh")?;
            std::process::exit(status.code().unwrap_or(1));
        }
        emit(&format!("ssh {}\n", argv.join(" ")));
        return Ok(());
    }

    if let Some(url) = game.web_url(login_level) {
        if run {
            let status = std::process::Command::new("xdg-open")
                .arg(&url)
                .status()
                .context("could not open a browser (xdg-open)")?;
            if !status.success() {
                bail!("xdg-open failed");
            }
        } else {
            emit(&format!("{url}\n"));
        }
        return Ok(());
    }

    bail!(
        "{} has no online server\nit is a released game: see: otw info {}",
        game.name,
        game.key
    )
}

fn cmd_start(cfg: &Cfg) {
    let mut out = String::new();
    out.push_str(&cfg.style.paint(St::Bold, "Suggested order"));
    out.push('\n');
    out.push_str(&cfg.style.paint(
        St::Dim,
        "Start at the beginning and move on when you feel comfortable.",
    ));
    out.push_str("\n\n");

    let steps: [(&str, &str); 4] = [
        ("bandit", "Unix/Linux basics, start here"),
        (
            "natas",
            "web security, or krypton (crypto) or leviathan (reverse engineering)",
        ),
        (
            "narnia",
            "binary exploitation, then behemoth → utumno → maze",
        ),
        (
            "vortex",
            "more binary exploitation when you want a bigger challenge",
        ),
    ];
    for (index, (game, note)) in steps.iter().enumerate() {
        out.push_str(&format!(
            "  {}  {:<10} {}\n",
            cfg.style.paint(St::Dim, &format!("{}.", index + 1)),
            cfg.style.paint(St::Accent, game),
            note
        ));
    }
    emit(&out);
}

#[derive(Clone)]
struct Target {
    game: usize,
    page: u32,
    path: String,
}

struct Hit {
    game: usize,
    page: u32,
    before: String,
    matched: String,
    after: String,
}

#[derive(Serialize)]
struct SearchHitJson {
    game: String,
    name: String,
    level: Option<u32>,
    page: u32,
    title: String,
    url: String,
    snippet: String,
}

#[derive(Serialize)]
struct SearchJson {
    query: String,
    results: Vec<SearchHitJson>,
}

struct SearchArgs<'a> {
    only_game: Option<&'a str>,
    jobs: usize,
    limit: usize,
    json: bool,
}

fn cmd_search(
    client: &Client,
    games: &[Game],
    query: &str,
    args: SearchArgs<'_>,
    cfg: &Cfg,
) -> Result<()> {
    let SearchArgs {
        only_game,
        jobs,
        limit,
        json,
    } = args;
    let query = query.trim();
    if query.is_empty() {
        bail!("the search query is empty");
    }

    let mut selected: Vec<usize> = Vec::new();
    if let Some(name) = only_game {
        let game = Game::find(games, name)?;
        let _ = game.require_levels()?;
        let index = games
            .iter()
            .position(|candidate| candidate.key == game.key)
            .expect("game came from the list");
        selected.push(index);
    } else {
        for (index, game) in games.iter().enumerate() {
            if game.last.is_some() {
                selected.push(index);
            }
        }
    }

    let mut targets = Vec::new();
    for &index in &selected {
        let game = &games[index];
        let last = game.last.expect("selected games have levels");
        for page in 0..=last {
            targets.push(Target {
                game: index,
                page,
                path: game.level_path(page),
            });
        }
    }

    let total = targets.len();
    if total == 0 {
        bail!("no level pages to search");
    }

    let show_progress = style::stderr_is_tty();
    if show_progress {
        eprint!("searching {total} pages...");
    }

    let regex = regex::RegexBuilder::new(&regex::escape(query))
        .case_insensitive(true)
        .build()?;
    let queue = Mutex::new(VecDeque::from(targets));
    let hits = Mutex::new(Vec::<Hit>::new());
    let failures = Mutex::new(Vec::<(String, String)>::new());
    let done = AtomicUsize::new(0);
    let workers = jobs.clamp(1, 16);

    std::thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| {
                loop {
                    let target = {
                        let mut queue = queue.lock().unwrap();
                        queue.pop_front()
                    };
                    let Some(target) = target else {
                        break;
                    };

                    let result = (|| -> Result<Option<Hit>> {
                        let raw = client.get(&target.path, PAGE_TTL)?;
                        let page = html::parse_page(&raw);
                        let mut found_flat = None;
                        for section in &page.sections {
                            let flat = section
                                .text
                                .split_whitespace()
                                .collect::<Vec<_>>()
                                .join(" ");
                            if let Some(found) = regex.find(&flat) {
                                let (start, end) = (found.start(), found.end());
                                found_flat = Some((flat, start, end));
                                break;
                            }
                        }
                        let Some((flat, start, end)) = found_flat else {
                            return Ok(None);
                        };
                        let raw_before = &flat[..start];
                        let truncated = raw_before.chars().count() > 70;
                        let before: String = if truncated {
                            raw_before
                                .chars()
                                .rev()
                                .take(70)
                                .collect::<Vec<_>>()
                                .into_iter()
                                .rev()
                                .collect()
                        } else {
                            raw_before.to_string()
                        };
                        let before = if truncated {
                            format!("…{}", before.trim_start())
                        } else {
                            before
                        };
                        let rest = &flat[end..];
                        let after_truncated = rest.chars().count() > 120;
                        let after = if after_truncated {
                            format!("{}…", rest.chars().take(120).collect::<String>())
                        } else {
                            rest.to_string()
                        };
                        Ok(Some(Hit {
                            game: target.game,
                            page: target.page,
                            before,
                            matched: flat[start..end].to_string(),
                            after,
                        }))
                    })();

                    match result {
                        Ok(Some(hit)) => hits.lock().unwrap().push(hit),
                        Ok(None) => {}
                        Err(error) => failures
                            .lock()
                            .unwrap()
                            .push((target.path.clone(), format!("{error:#}"))),
                    }

                    let count = done.fetch_add(1, Ordering::Relaxed) + 1;
                    if show_progress {
                        eprint!("\rsearching {count}/{total} pages...");
                    }
                }
            });
        }
    });

    if show_progress {
        eprint!("\r\x1b[K");
    }

    let failures = failures.into_inner().unwrap();
    for (path, error) in failures.iter().take(3) {
        eprintln!("warning: /{path}: {error}");
    }
    if failures.len() > 3 {
        eprintln!("warning: {} more pages failed", failures.len() - 3);
    }

    let mut hits = hits.into_inner().unwrap();
    hits.sort_by_key(|hit| (hit.game, hit.page));
    let found_count = hits.len();
    if limit > 0 {
        hits.truncate(limit);
    }

    if json {
        let results = hits
            .iter()
            .map(|hit| SearchHitJson {
                game: games[hit.game].key.clone(),
                name: games[hit.game].name.clone(),
                level: games[hit.game].task_level(hit.page),
                page: hit.page,
                title: games[hit.game].level_title(hit.page),
                url: games[hit.game].level_url(hit.page),
                snippet: format!("{}{}{}", hit.before, hit.matched, hit.after),
            })
            .collect();
        print_json(&SearchJson {
            query: query.to_string(),
            results,
        })?;
        if found_count == 0 {
            bail!("no matches for '{query}'");
        }
        return Ok(());
    }

    if hits.is_empty() {
        bail!("no matches for '{query}'");
    }

    let mut out = String::new();
    let mut current_game = usize::MAX;
    for hit in &hits {
        if hit.game != current_game {
            if current_game != usize::MAX {
                out.push('\n');
            }
            out.push_str(&cfg.style.paint(St::Accent, &games[hit.game].name));
            out.push('\n');
            current_game = hit.game;
        }
        out.push_str(&format!(
            "  {}\n",
            cfg.style
                .paint(St::Bold, &games[hit.game].level_title(hit.page))
        ));

        let line = format!("{}{}{}", hit.before, hit.matched, hit.after);
        out.push_str(&format!("    {line}\n"));
    }

    if found_count > hits.len() {
        out.push_str(&format!(
            "\n{}\n",
            cfg.style.paint(
                St::Dim,
                &format!("showing {} of {found_count} matches", hits.len())
            )
        ));
    }

    emit(&out);
    Ok(())
}

#[derive(Serialize)]
struct PageJson {
    path: String,
    url: String,
    title: String,
    sections: Vec<Section>,
    links: Vec<Link>,
    text: String,
}

fn include_regex() -> &'static regex::Regex {
    static REGEX: OnceLock<regex::Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        regex::Regex::new(r#"(?s)<script[^>]*>\s*renderCode\(\s*"([^"]+)"\s*\)\s*</script>"#)
            .expect("valid include regex")
    })
}

fn include_paths(html: &str, page_path: &str) -> Vec<String> {
    let directory = match page_path.rfind('/') {
        Some(index) => &page_path[..=index],
        None => "",
    };
    include_regex()
        .captures_iter(html)
        .map(|captures| format!("{directory}{}", &captures[1]))
        .collect()
}

fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// The site loads some level files (source code, instructions) with a
/// `renderCode("...")` JavaScript call. Fetch those files and inline them so
/// the terminal version is complete.
fn expand_includes(client: &Client, html: &str, page_path: &str) -> String {
    let directory = match page_path.rfind('/') {
        Some(index) => &page_path[..=index],
        None => "",
    };
    let mut out = String::with_capacity(html.len() + 4096);
    let mut last = 0;
    for captures in include_regex().captures_iter(html) {
        let whole = captures.get(0).expect("group 0");
        let name = &captures[1];
        out.push_str(&html[last..whole.start()]);
        let path = format!("{directory}{name}");
        match client.get(&path, PAGE_TTL) {
            Ok(code) => {
                out.push_str("<pre><code>");
                out.push_str(&escape_html(code.trim_end()));
                out.push_str("</code></pre>");
            }
            Err(error) => {
                out.push_str(&format!(
                    "<p><em>(could not load included file {}: {})</em></p>",
                    escape_html(name),
                    escape_html(&format!("{error:#}"))
                ));
            }
        }
        last = whole.end();
    }
    out.push_str(&html[last..]);
    out
}

fn page_path(input: &str) -> Result<String> {
    let mut path = input.trim();
    if let Some(rest) = path
        .strip_prefix("https://overthewire.org")
        .or_else(|| path.strip_prefix("http://overthewire.org"))
    {
        path = rest;
    } else if path.starts_with("http://") || path.starts_with("https://") {
        bail!("only pages on overthewire.org are supported");
    }
    let path = path.trim_start_matches('/');
    if path.is_empty() {
        bail!("missing page path");
    }
    if path.contains("..") {
        bail!("invalid page path");
    }
    Ok(path.to_string())
}

fn cmd_page(client: &Client, path: &str, cfg: &Cfg, json: bool) -> Result<()> {
    let path = page_path(path)?;
    let raw = client
        .get(&path, PAGE_TTL)
        .with_context(|| format!("loading /{path}"))?;
    let raw = expand_includes(client, &raw, &path);

    if json {
        let page = html::parse_page(&raw);
        return print_json(&PageJson {
            path: path.clone(),
            url: format!("{}/{}", client::BASE, path),
            title: page.title,
            sections: page.sections,
            links: page.links,
            text: page.text,
        });
    }

    let body = html::render_page(&raw, &cfg.render_opts());
    if body.is_empty() {
        bail!("/{path} has no readable main content");
    }
    emit(&body);
    Ok(())
}

#[derive(Serialize)]
struct CacheStatusJson {
    path: String,
    files: usize,
    bytes: u64,
    oldest_age_seconds: Option<u64>,
    newest_age_seconds: Option<u64>,
}

fn cmd_cache(command: CacheCommand, cfg: &Cfg) -> Result<()> {
    match command {
        CacheCommand::Status { json } => {
            let cache = Cache::new()?;
            let stats = cache.stats()?;
            let now = std::time::SystemTime::now();
            if json {
                return print_json(&CacheStatusJson {
                    path: cache.root().display().to_string(),
                    files: stats.files,
                    bytes: stats.bytes,
                    oldest_age_seconds: stats
                        .oldest
                        .and_then(|time| now.duration_since(time).ok())
                        .map(|age| age.as_secs()),
                    newest_age_seconds: stats
                        .newest
                        .and_then(|time| now.duration_since(time).ok())
                        .map(|age| age.as_secs()),
                });
            }
            let mut out = String::new();
            out.push_str(&footer_row(
                cfg,
                "Path",
                &cache.root().display().to_string(),
            ));
            out.push_str(&footer_row(cfg, "Files", &stats.files.to_string()));
            out.push_str(&footer_row(cfg, "Size", &human_bytes(stats.bytes)));
            out.push_str(&footer_row(
                cfg,
                "Oldest",
                &stats
                    .oldest
                    .map(human_age)
                    .unwrap_or_else(|| "-".to_string()),
            ));
            out.push_str(&footer_row(
                cfg,
                "Newest",
                &stats
                    .newest
                    .map(human_age)
                    .unwrap_or_else(|| "-".to_string()),
            ));
            emit(&out);
            Ok(())
        }
        CacheCommand::Clear => {
            let cache = Cache::new()?;
            let stats = cache.clear()?;
            emit(&format!(
                "removed {} files ({}) from {}\n",
                stats.files,
                human_bytes(stats.bytes),
                cache.root().display()
            ));
            Ok(())
        }
        CacheCommand::Fetch { games, all, jobs } => {
            if games.is_empty() && !all {
                bail!("specify at least one game, or --all\n\nexample: otw cache fetch --all");
            }
            let client = Client::new(cfg.offline, cfg.refresh)?;
            let all_games = games::load(&client)?;

            let wanted: Vec<&Game> = if games.is_empty() {
                all_games.iter().collect()
            } else {
                let mut wanted = Vec::new();
                for name in &games {
                    wanted.push(Game::find(&all_games, name)?);
                }
                wanted
            };

            let mut targets: Vec<String> = Vec::new();
            for game in &wanted {
                targets.push(game.info_path());
                if let Some(last) = game.last {
                    for page in 0..=last {
                        targets.push(game.level_path(page));
                    }
                }
            }
            let total = targets.len();
            if total == 0 {
                bail!("nothing to fetch");
            }

            let show_progress = style::stderr_is_tty();
            if show_progress {
                eprint!("fetching {total} pages...");
            }
            let queue = Mutex::new(VecDeque::from(targets));
            let failures = Mutex::new(Vec::<(String, String)>::new());
            let ok = AtomicUsize::new(0);
            let done = AtomicUsize::new(0);
            let workers = jobs.clamp(1, 16);

            std::thread::scope(|scope| {
                for _ in 0..workers {
                    scope.spawn(|| {
                        loop {
                            let target = {
                                let mut queue = queue.lock().unwrap();
                                queue.pop_front()
                            };
                            let Some(target) = target else {
                                break;
                            };
                            match client.get(&target, PAGE_TTL) {
                                Ok(html) => {
                                    for include in include_paths(&html, &target) {
                                        if let Err(error) = client.get(&include, PAGE_TTL) {
                                            failures
                                                .lock()
                                                .unwrap()
                                                .push((include.clone(), format!("{error:#}")));
                                        }
                                    }
                                    ok.fetch_add(1, Ordering::Relaxed);
                                }
                                Err(error) => failures
                                    .lock()
                                    .unwrap()
                                    .push((target.clone(), format!("{error:#}"))),
                            }
                            let count = done.fetch_add(1, Ordering::Relaxed) + 1;
                            if show_progress {
                                eprint!("\rfetching {count}/{total} pages...");
                            }
                        }
                    });
                }
            });

            if show_progress {
                eprint!("\r\x1b[K");
            }

            let failures = failures.into_inner().unwrap();
            for (path, error) in failures.iter().take(3) {
                eprintln!("warning: /{path}: {error}");
            }
            if failures.len() > 3 {
                eprintln!("warning: {} more pages failed", failures.len() - 3);
            }

            let ok = ok.into_inner();
            emit(&format!(
                "cached {}/{} pages in {}\n",
                ok,
                total,
                client.cache().root().display()
            ));
            if ok == 0 {
                bail!("could not cache any pages");
            }
            Ok(())
        }
    }
}
