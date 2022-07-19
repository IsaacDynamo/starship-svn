use clap::Parser;
use std::io::Error;
use std::path::Path;
use svn_cmd::{SvnCmd, SvnError, SvnInfo};
use url::{ParseError, Url};

const ABOUT: &str = concat!(
r#"Print SVN branch name when in a working copy.

SVN only has a branch name by convention. The name is derived from the 'svn info' URL. "#,
"This is done with the assumption that the repo uses the conventional trunk/branches/tags repository layout. ",
"If the URL path contains a folder named 'trunk', the branch name is trunk. ",
r#"If the URL path contains a folder named 'branches' or 'tags' the next folder is considered the branch name, if it is not blacklisted.

If your branch named folder is not a direct child of 'branches' or 'tags' the inbetween folders can be blacklisted. "#,
r#"The algorithm continues to scans right for a branch name until a non blacklisted folder name is found.

Add the following to your starship.toml.

[custom.svn]
description = 'SVN branch name'
command     = 'starship-svn'
when        = 'starship-svn'
format      = 'on [$symbol$output]($style) '
symbol      = ' '
style       = 'bold purple'"#);

#[derive(Parser, Debug)]
#[clap(version, about = ABOUT, long_about = ABOUT)]
struct Args {
    /// Print SVN working copy root folder
    #[clap(short, long)]
    root: bool,

    /// Blacklisted branch names
    #[clap(short, long, value_delimiter = ',', value_name = "NAME,..")]
    blacklist: Vec<String>,
}

#[derive(Debug)]
enum AppErr {
    Svn(SvnError),
    Url(ParseError),
    Io(Error),
    UnexpectedRepoLayout,
    BranchNameNotFound,
    PathNotUtf8,
    ParseError,
}

impl From<SvnError> for AppErr {
    fn from(e: SvnError) -> Self {
        Self::Svn(e)
    }
}

impl From<ParseError> for AppErr {
    fn from(e: ParseError) -> Self {
        Self::Url(e)
    }
}

impl From<Error> for AppErr {
    fn from(e: Error) -> Self {
        Self::Io(e)
    }
}

/// Try to determine branch name from svn info.
fn branch(info: &SvnInfo, blacklist: &[String]) -> Result<String, AppErr> {
    // Convert url
    let url = Url::parse(&info.entry.url)?;
    let path = Path::new(url.path());

    // Walk over the path
    let mut iter = path.iter();
    while let Some(folder) = iter.next() {
        let lower_folder = folder.to_ascii_lowercase();

        // On trunk?
        if lower_folder == "trunk" {
            let folder = folder.to_str().ok_or(AppErr::PathNotUtf8)?;
            return Ok(folder.to_string());
        }

        // Found branches or tags folder, next folder will be considered the branch name
        if lower_folder == "branches" || lower_folder == "tags" {
            while let Some(folder) = iter.next() {
                let folder = folder.to_str().ok_or(AppErr::PathNotUtf8)?;

                // Skip this folder if it is in the blacklist
                if blacklist.iter().any(|x| x == folder) {
                    continue;
                }

                // First non blacklisted folder after 'branches' or 'tags'
                return Ok(folder.to_string());
            }
            return Err(AppErr::BranchNameNotFound);
        }
    }

    Err(AppErr::UnexpectedRepoLayout)
}

/// Find the root of the working copy
fn root(info: &SvnInfo, pwd: &Path) -> Result<String, AppErr> {
    let rel = &info.entry.relative_url;

    // Prepare reverse iterators
    let mut rel = Path::new(rel).iter().rev();
    let mut pwd = pwd.iter().rev();

    // Loop until we find the root
    loop {
        // Get folders
        let pwd_part = pwd.next().ok_or(AppErr::ParseError)?;
        let rel_part = rel.next().ok_or(AppErr::ParseError)?;

        // Reached end of relative path, so we are at the working copy root.
        if rel_part == "^" {
            return Ok(pwd_part.to_str().ok_or(AppErr::PathNotUtf8)?.to_string());
        }

        // As long as we step backwards through the path the folders must match.
        if pwd_part != rel_part {
            return Err(AppErr::ParseError);
        }
    }
}

fn main() -> Result<(), AppErr> {
    let args = Args::parse();

    // Prepare target path
    let pwd = std::env::current_dir()?;
    let target = pwd.to_str().ok_or(AppErr::PathNotUtf8)?;

    // Get svn info
    let svn = SvnCmd::new(None, None);
    let info = svn.info(target)?;

    if args.root {
        println!("{}", root(&info, &pwd)?);
    } else {
        println!("{}", branch(&info, &args.blacklist)?);
    }

    Ok(())
}
