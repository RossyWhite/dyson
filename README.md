# dyson

dyson is a command-line tool that helps you delete unused ECR images from your registry.
It provides several commands to manage the deletion process.

## Installation

To use dyson, you need to have Rust and Cargo installed. If you don't have them installed, you can follow the
official Rust installation guide at [https://www.rust-lang.org/tools/install](https://www.rust-lang.org/tools/install).

Once you have Rust and Cargo installed, you can install dyson by running the following command:

```bash
cargo install dyson
```

Or, you can download the latest release from the [GitHub releases page](https://github.com/RossyWhite/dyson/releases).

## Usage

```bash
dyson [OPTIONS] <COMMAND>
```

Commands:

- `init`: Generate a configuration file
- `plan`: Make a deletion plan according to the config
- `apply`: Delete ECR images according to the config
- `help`: Print this message or the help of the given subcommand(s)

Options:

- `-c, --config <FILE>`: Path to the configuration file. Default: `dyson.yaml`

## Configuration

Dyson requires a configuration file that specifies the rules for identifying unused images. By default, the
configuration file is named `dyson.yaml`.

You can customize the configuration file to fit your specific needs. The configuration file uses YAML format and should
contain the following information:

```yaml
registry:
  name: my-registry
  profile_name: profile1
  excludes:
    - exclude/*
  filters:
    - pattern: '*'
      days_after: 30
      ignore_tag_patterns:
        - latest

scans:
  - name: scan-target
    profile_name: profile2

notification:
  slack:
    webhook_url: https://hooks.slack.com/services/xxx/yyy/zzz
    username: dyson-bot
    channel: random
```

### Registry Configuration

The `registry` section defines the settings for your ECR registry:

- `name` (optional): The name of your ECR registry.
- `profile_name`: The AWS profile name to use for authentication when accessing the registry.
- `excludes` (optional): A list of repository patterns to exclude from the deletion process. Wildcards (`*`) are
  supported.
- `filters` (optional): A list of filters images based on their last push date and tags.
    - `pattern`: The repository pattern to match. Wildcards (`*`) are supported.
    - `days_after` (optional): The number of days after pushed which an image is considered target for deletion.
    - `ignore_tag_patterns` (optional): A list of tag patterns to ignore from target for deletion. Wildcards (`*`) are
      supported.

### Scans Configuration

In the scan process, dyson will scan the accounts for images that are used by

- Lambda functions
- ECS Services
- ECS Task Definitions(currently latest two revisions are considered as used)

The `scans` section defines the scans target accounts.

- `name` (optional): The name of the scan.
- `profile_name`: The AWS profile name to use for authentication when accessing the account.

You can define multiple scans in the `scans` section if needed.

### Notifier Configuration

Dyson provide a simple notification mechanism to notify the result. Currently, only Slack is supported.

- `webhook_url`: The Slack webhook URL to which the notifications will be sent.
- `username (optional)`: The username to display for the notification.
- `channel (optional)`: The Slack channel or user ID to which the notifications will be sent.
