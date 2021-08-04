# telegram-beancount-bot

A telegram [beancount][] accounting bot.

Assumes:
- Accounts are in `accounts.bean`.
- Configured a git remote, and the default branch is tracked to a remote branch.
- Transactions are placed in `txs/{year}/{month:02}.bean`.

[beancount]: https://github.com/beancount/beancount

## Usage

- `cp git-hooks/pre-commit .git/hooks`
- Configure
- `cargo run --release`
- Send `/auth <secret>` to authorize yourself

## License

AGPL. See the `LICENSE` file for more detail.
