use std::fmt;
use std::fs::{self, File};
use std::io::{self, BufRead, BufReader, Seek, SeekFrom, Write};
use std::path::Path;

use anyhow::{anyhow, Context, Result};
use chrono::NaiveDate;
use rust_decimal::Decimal;

use crate::utils::{escape_string, last_component, naive_today};

#[derive(Debug)]
pub struct Transaction<'ac, 'am> {
    date: NaiveDate,
    payee: Option<String>,
    narration: String,
    tags: Vec<String>,
    postings: Vec<Posting<'ac, 'am>>,
}

#[derive(Debug)]
pub struct Posting<'ac, 'am> {
    account: &'ac str,
    amount: Amount<'am>,
}

#[derive(Debug, Clone)]
pub struct Amount<'a> {
    pub number: Decimal,
    pub currency: &'a str,
}

/// Determines whether `account` matches the lowercased search term `term`. If the term contains
/// whitespace, all subterms in the term has to appear in the account.
fn account_matches(account: &str, term: &str) -> bool {
    let loweraccount = account.to_lowercase();
    term.split_ascii_whitespace()
        .all(|t| loweraccount.contains(t))
}

fn filter_account<'a>(
    accounts: &'a [String],
    term: &str,
    pred: impl Fn(&&String) -> bool,
) -> Result<&'a String> {
    // 1. last component
    // 2. full account name
    let term = term.to_lowercase();
    let matched: Vec<_> = accounts
        .iter()
        .filter(|ac| account_matches(ac, &term) && pred(ac))
        .collect();
    match matched.len() {
        0 => Err(anyhow!("No matched account")),
        1 => Ok(matched[0]),
        _ => {
            // check if the last components of accounts has a unique match
            let last_match: Vec<_> = matched
                .iter()
                .filter(|ac| account_matches(last_component(ac), &term))
                .collect();
            match last_match.len() {
                0 => Err(anyhow!("More than one matched account: {:?}", matched)),
                1 => Ok(last_match[0]),
                _ => Err(anyhow!(
                    "More than one last-component matched account: {:?}",
                    last_match
                )),
            }
        }
    }
}

impl<'ac, 'am: 'ac> Transaction<'ac, 'am> {
    /// Parses a transaction from a command.
    /// [>Payee] [#Tag ...] Amount Account ExpAccount Narration
    pub fn today_from_command(
        cmds: &'am [String],
        accounts: &'ac [String],
        default_currency: &'am str,
    ) -> Result<Self> {
        let mut iter = cmds.iter().peekable();
        let payee = iter
            .next_if(|x| x.starts_with('>'))
            .map(|s| s[1..].to_string());

        let mut tags = Vec::new();
        while let Some(tag) = iter.next_if(|x| x.starts_with('#')) {
            tags.push(tag.to_string());
        }

        let cmd_amount = iter
            .next()
            .ok_or_else(|| anyhow!("Not enough arguments: amount"))?;
        let cmd_spd_acc = iter
            .next()
            .ok_or_else(|| anyhow!("Not enough arguments: account"))?;
        let cmd_exp_acc = iter
            .next()
            .ok_or_else(|| anyhow!("Not enough arguments: expense account"))?;
        let narration = iter.map(|x| x.as_str()).collect::<Vec<_>>().join(" ");
        // if narration.is_empty() {
        //     return Err(anyhow!("Empty narration"));
        // }
        let amount = Amount::from_str(cmd_amount, default_currency)
            .ok_or_else(|| anyhow!("Invalid amount {}", cmd_amount))?;

        let account = filter_account(accounts, cmd_spd_acc, |x| !x.starts_with("Expenses:"))
            .context("Invalid spend account")?;
        let expense_account = filter_account(accounts, cmd_exp_acc, |x| x.starts_with("Expenses:"))
            .context("Invalid expense account")?;
        let postings = vec![
            Posting::new(expense_account, amount.clone()),
            Posting::new(account, -amount),
        ];

        let date = naive_today();

        Ok(Self {
            date,
            payee,
            narration,
            tags,
            postings,
        })
    }
}

/// Appends `text` to a file
pub fn append_to_file(text: &str, filename: impl AsRef<Path>) -> io::Result<()> {
    let parent = filename
        .as_ref()
        .parent()
        .expect("there should be a parent");
    if !parent.exists() {
        fs::create_dir(parent)?;
    }
    let mut fw = fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(filename)?;
    // have to seek end, otherwise the stream_position method will return 0
    fw.seek(SeekFrom::End(0))?;
    if fw.stream_position()? != 0 {
        writeln!(fw)?;
    }
    writeln!(fw, "{}", text)?;
    Ok(())
}

impl<'ac, 'am> Posting<'ac, 'am> {
    pub fn new(account: &'ac str, amount: Amount<'am>) -> Self {
        Self { account, amount }
    }
}

impl<'a> Amount<'a> {
    pub fn from_str(s: &'a str, default_currency: &'a str) -> Option<Self> {
        let regex = regex!(r"^([0-9.]+)\s*([A-Z][A-Z0-9'._-]{0,22}[A-Z0-9])?$");
        let caps = regex.captures(s)?;
        let number: Decimal = caps.get(1).and_then(|n| n.as_str().parse().ok())?;
        let currency = caps.get(2).map_or(default_currency, |c| c.as_str());
        Some(Self { number, currency })
    }
}

impl<'a> std::ops::Neg for Amount<'a> {
    type Output = Self;
    fn neg(self) -> Self::Output {
        Self {
            number: -self.number,
            currency: self.currency,
        }
    }
}

// Displays
impl<'ac, 'am> fmt::Display for Transaction<'ac, 'am> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // first line
        write!(f, "{} *", self.date.format("%F"))?;
        if let Some(ref payee) = self.payee {
            write!(f, r#" "{}""#, escape_string(payee))?;
        }
        write!(f, r#" "{}""#, escape_string(&self.narration))?;
        for tag in self.tags.iter() {
            write!(f, " {}", tag)?;
        }
        writeln!(f)?;

        // postings
        for posting in self.postings.iter() {
            writeln!(f, "    {}", posting)?;
        }
        // TODO: trim out the last \n
        Ok(())
    }
}

impl<'ac, 'am> fmt::Display for Posting<'ac, 'am> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.account, self.amount)
    }
}

impl<'a> fmt::Display for Amount<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.number, self.currency)
    }
}

pub fn get_accounts(path: impl AsRef<Path>) -> io::Result<Vec<String>> {
    // TODO: categorize accounts to accounts/*.bean
    // assuming all accounts are in {root}/accounts.bean
    let account_path = BufReader::new(File::open(path.as_ref().join("accounts.bean"))?);
    let mut ret = Vec::new();
    for line in account_path.lines() {
        let line = line?;
        let xs = line
            .split_ascii_whitespace()
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        if xs.len() < 3 || xs[0].starts_with(';') {
            continue;
        }
        match xs[1].as_str() {
            "open" => {
                // sadly, we have to clone here
                //   https://users.rust-lang.org/t/why-cant-move-element-of-vector/30454/4
                ret.push(xs[2].clone());
            }
            "close" => {
                // TODO: remove closed accounts
            }
            _ => {}
        }
    }
    Ok(ret)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_matches() {
        assert!(account_matches("Expenses:Transport:Public:Bus", "bus"));
        assert!(account_matches("Expenses:Transport:Bus", "transp bus"));
        assert!(account_matches("Expenses:Transport:Bus", " transp  bus "));
    }

    #[test]
    fn test_filter() {
        let accounts: Vec<_> = vec![
            "Assets:Cash:CNY",
            "Assets:Cash:USD",
            "Expenses:International:Fees",
            "Expenses:Food:Groceries",
            "Expenses:Health:Dental:Insurance",
            "Expenses:Health:Life:GroupTermLife",
            "Expenses:Health:Medical:Insurance",
            "Expenses:Health:Vision:Insurance",
            "Expenses:Home:Internet",
            "Expenses:Home:Phone",
            "Expenses:Home:Rent",
        ]
        .iter()
        .map(ToString::to_string)
        .collect();
        let pred = |s: &&String| s.starts_with("Expenses:");
        assert!(
            format!("{}", filter_account(&accounts, "insur", pred).unwrap_err())
                .starts_with("More than one last-component matched account: ")
        );
        assert!(
            format!("{}", filter_account(&accounts, "health", pred).unwrap_err())
                .starts_with("More than one matched account: ")
        );
        // whole account unique match
        assert_eq!(
            filter_account(&accounts, "dental", pred).unwrap(),
            "Expenses:Health:Dental:Insurance"
        );
        // last component unique match
        assert_eq!(
            filter_account(&accounts, "inter", pred).unwrap(),
            "Expenses:Home:Internet"
        );
        // multiple terms match
        assert_eq!(
            filter_account(&accounts, "med insur", pred).unwrap(),
            "Expenses:Health:Medical:Insurance"
        );
    }
}
