use std::process::{self, ExitStatus};

use regex::Regex;

use crate::{
    cli::CommitCommand,
    conventional::{config::Type, CommitParser, Config},
    Command, Error,
};

#[cfg(target_family = "unix")]
extern crate skim;
#[cfg(target_family = "unix")]
use skim::prelude::*;
#[cfg(target_family = "unix")]
use std::io::Cursor;

#[cfg(not(target_family = "unix"))]
use dialoguer::{theme::ColorfulTheme, FuzzySelect};

fn read_single_line(
    theme: &impl dialoguer::theme::Theme,
    prompt: &str,
    default: &str,
) -> Result<String, Error> {
    Ok(dialoguer::Input::with_theme(theme)
        .with_prompt(prompt)
        .default(default.to_string())
        .allow_empty(true)
        .interact_text()?)
}

fn make_commit_message(
    Dialog {
        r#type,
        scope,
        description,
        body,
        breaking_change,
        issues,
    }: &Dialog,
    breaking: bool,
) -> String {
    let mut msg = r#type.to_string();
    if !scope.is_empty() {
        msg.push('(');
        msg.push_str(scope.as_str());
        msg.push(')');
    }
    if breaking || !breaking_change.is_empty() {
        msg.push('!');
    }
    msg.push_str(": ");
    msg.push_str(description.as_str());
    if !body.is_empty() {
        msg.push_str("\n\n");
        msg.push_str(body.as_str())
    }
    if !breaking_change.is_empty() {
        msg.push_str("\n\n");
        msg.push_str(format!("BREAKING CHANGE: {}", breaking_change).as_str());
    }
    if !issues.is_empty() {
        msg.push_str("\n\n");
        msg.push_str(format!("Refs: {}", issues).as_str());
    }
    msg
}

impl CommitCommand {
    fn commit(&self, msg: String) -> Result<ExitStatus, Error> {
        // build the command
        let mut cmd = process::Command::new("git");
        cmd.args(&["commit", "-m", msg.as_str()]);

        if !self.extra_args.is_empty() {
            cmd.args(&self.extra_args);
        }
        Ok(cmd.status()?)
    }
}

fn read_scope(
    theme: &impl dialoguer::theme::Theme,
    default: &str,
    scope_regex: Regex,
) -> Result<String, Error> {
    let result: String = dialoguer::Input::<String>::with_theme(theme)
        .with_prompt("scope")
        .validate_with(move |input: &String| match scope_regex.is_match(input) {
            true => Ok(()),
            false => {
                if input.is_empty() {
                    Ok(())
                } else {
                    Err(format!("scope does not match regex {:?}", scope_regex))
                }
            }
        })
        .default(default.to_string())
        .allow_empty(true)
        .interact_text()?;
    Ok(result)
}

fn read_description(
    theme: &impl dialoguer::theme::Theme,
    default: String,
) -> Result<String, Error> {
    let result: String = dialoguer::Input::<String>::with_theme(theme)
        .with_prompt("description")
        .validate_with(|input: &String| {
            if input.len() < 5 {
                Err("Description needs a length of at least 5 characters")
            } else {
                Ok(())
            }
        })
        .default(default)
        .allow_empty(false)
        .interact_text()?;
    Ok(result)
}

fn edit_message(msg: &str) -> Result<String, Error> {
    Ok(dialoguer::Editor::new()
        .require_save(true)
        .edit(msg)?
        .unwrap_or_default()
        .lines()
        .filter(|line| !line.starts_with('#'))
        .collect::<Vec<&str>>()
        .join("\n")
        .trim()
        .to_owned())
}

struct Dialog {
    r#type: String,
    scope: String,
    description: String,
    body: String,
    breaking_change: String,
    issues: String,
}

impl Default for Dialog {
    fn default() -> Self {
        Self {
            r#type: String::default(),
            scope: String::default(),
            description: String::default(),
            body: "# A longer commit body MAY be provided after the short description, \n\
                   # providing additional contextual information about the code changes. \n\
                   # The body MUST begin one blank line after the description. \n\
                   # A commit body is free-form and MAY consist of any number of newline separated paragraphs.\n".to_string(),
            breaking_change: String::default(),
            issues: String::default(),
        }
    }
}

impl Dialog {
    fn select_type(
        theme: &impl dialoguer::theme::Theme,
        selected: &str,
        types: &[Type],
    ) -> Result<String, Error> {
        let mut sel: String = "".to_string();

        #[cfg(target_family = "unix")]
        {
            // `SkimItemReader` is a helper to turn any `BufRead` into a stream of `SkimItem`
            // `SkimItem` was implemented for `AsRef<str>` by default
            let item_reader = SkimItemReader::default();

            let mut input = String::new();
            for c in types {
                input.push_str(&c.r#type);
                input.push('\n');
            }
            let items = item_reader.of_bufread(Cursor::new(input));

            // use dialoguer::theme::ColorfulTheme;
            // let mut header : String = "type: ".to_owned();
            // header.push_str(ColorfulTheme::default().active_item_prefix.to_string().as_str());
            // .header(Some(header.as_str()))
            let options = SkimOptionsBuilder::default()
                .min_height("10".to_string())
                .prompt("? type () › ".to_string())
                .height("25%".to_string())
                .multi(false)
                .reverse(true)
                .build()
                .unwrap();
            // `run_with` would read and show items from the stream
            let selected_items = Skim::run_with(&options, Some(items))
                .map(|out| out.selected_items)
                .unwrap_or_default();

            if !selected_items.is_empty() {
                let item = selected_items.get(0).unwrap();
                sel = item.output().to_string();
            } else if !selected.is_empty() {
                sel = selected.to_string();
            }
        }

        #[cfg(not(target_family = "unix"))]
        {
            // windows
            let mut selections: Vec<&str> = Vec::new();
            for c in types {
                selections.push(&c.r#type);
            }

            let selected_item = FuzzySelect::with_theme(&ColorfulTheme::default())
                .with_prompt("Pick your flavor")
                .default(0)
                .items(&selections[..])
                .interact()
                .unwrap();

            if !selected_item.is_empty() {
                sel = selected_item;
            } else if !selected.is_empty() {
                sel = selected.to_string();
            }
        }

        if sel.is_empty() {
            sel = types[0].r#type.to_string();
        }

        let term: &console::Term = &console::Term::stderr();
        let mut buf = String::new();
        let _ = theme.format_input_prompt_selection(&mut buf, "type", &sel);
        term.write_line(buf.as_str())?;
        term.clear_line()?;

        Ok(sel)
    }

    // Prompt all
    fn wizard(
        config: &Config,
        parser: CommitParser,
        r#type: Option<String>,
        breaking: bool,
    ) -> Result<String, Error> {
        let mut dialog = Self::default();
        let theme = &dialoguer::theme::ColorfulTheme::default();
        let types = config.types.as_slice();
        let scope_regex = Regex::new(config.scope_regex.as_str()).expect("valid scope regex");

        // type
        let current_type = dialog.r#type.as_str();
        match (r#type.as_ref(), current_type) {
            (Some(t), "") if !t.is_empty() => dialog.r#type = t.to_owned(),
            (_, t) => {
                dialog.r#type = Self::select_type(theme, t, types)?;
            }
        }
        // scope
        dialog.scope = read_scope(theme, dialog.scope.as_ref(), scope_regex)?;
        // description
        dialog.description = read_description(theme, dialog.description)?;
        // breaking change
        dialog.breaking_change = read_single_line(
            theme,
            "optional BREAKING change",
            dialog.breaking_change.as_str(),
        )?;
        // issues
        dialog.issues = read_single_line(theme, "issues (e.g. #2, #8)", dialog.issues.as_str())?;

        loop {
            // finally make message
            let msg = make_commit_message(&dialog, breaking);
            let msg = edit_message(msg.as_str())?;
            match parser.parse(msg.as_str()).map(|_| msg) {
                Ok(msg) => break Ok(msg),
                Err(e) => {
                    eprintln!("ParseError: {}", e);
                    if !dialoguer::Confirm::new()
                        .with_prompt("Continue?")
                        .interact()?
                    {
                        break Err(Error::CancelledByUser);
                    }
                }
            }
        }
    }
}

impl Command for CommitCommand {
    fn exec(&self, config: Config) -> Result<(), Error> {
        let r#type = match (
            self.feat,
            self.fix,
            self.build,
            self.chore,
            self.ci,
            self.docs,
            self.style,
            self.refactor,
            self.perf,
            self.test,
        ) {
            (true, false, false, false, false, false, false, false, false, false) => {
                Some("feat".to_string())
            }
            (false, true, false, false, false, false, false, false, false, false) => {
                Some("fix".to_string())
            }
            (false, false, true, false, false, false, false, false, false, false) => {
                Some("build".to_string())
            }
            (false, false, false, true, false, false, false, false, false, false) => {
                Some("chore".to_string())
            }
            (false, false, false, false, true, false, false, false, false, false) => {
                Some("ci".to_string())
            }
            (false, false, false, false, false, true, false, false, false, false) => {
                Some("docs".to_string())
            }
            (false, false, false, false, false, false, true, false, false, false) => {
                Some("style".to_string())
            }
            (false, false, false, false, false, false, false, true, false, false) => {
                Some("refactor".to_string())
            }
            (false, false, false, false, false, false, false, false, true, false) => {
                Some("perf".to_string())
            }
            (false, false, false, false, false, false, false, false, false, true) => {
                Some("test".to_string())
            }
            _ => None,
        };
        let parser = CommitParser::builder()
            .scope_regex(config.scope_regex.clone())
            .build();
        let msg = Dialog::wizard(&config, parser, r#type, self.breaking)?;

        self.commit(msg)?;
        Ok(())
    }
}
