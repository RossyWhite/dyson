use std::path::Path;

use crate::config::DysonConfig;
use crate::dyson::Dyson;
use crate::summary::write_summary;

/// Dyson CLI
#[derive(clap::Parser)]
#[command(version)]
pub struct DysonCli {
    #[command(subcommand)]
    command: Commands,
    #[clap(flatten)]
    global_args: GlobalArgs,
}

impl DysonCli {
    /// Run the command
    pub async fn run(&self) -> Result<(), Box<dyn std::error::Error>> {
        match &self.command {
            Commands::Init(args) => self.run_init_command(args).await,
            Commands::Plan => self.run_plan_command().await,
            Commands::Apply => self.run_apply_command().await,
        }
    }

    /// Run the init command
    async fn run_init_command(&self, args: &InitArgs) -> Result<(), Box<dyn std::error::Error>> {
        let cfg = DysonConfig::example_config();
        if args.stdout {
            serde_yaml::to_writer(std::io::stdout(), &cfg)?;
            return Ok(());
        }

        let path = Path::new(&self.global_args.config_path);
        if path.exists() {
            return Ok(());
        }
        let f = std::fs::File::create(path)?;
        serde_yaml::to_writer(&f, &cfg)?;

        Ok(())
    }

    /// Run the plan command
    async fn run_plan_command(&self) -> Result<(), Box<dyn std::error::Error>> {
        let dyson = self.try_new_cleaner().await?;
        let targets = dyson.list_target_images().await?;

        let mut buf = Vec::new();
        write_summary(&targets, &mut std::io::BufWriter::new(&mut buf));
        let summary = String::from_utf8(buf)?;
        println!("Plan Result:\n{}", summary);
        dyson.notify_result("Plan Succeeded!!", targets).await?;
        Ok(())
    }

    /// Run the apply command
    async fn run_apply_command(&self) -> Result<(), Box<dyn std::error::Error>> {
        let dyson = self.try_new_cleaner().await?;
        let targets = dyson.list_target_images().await?;

        let mut buf = Vec::new();
        write_summary(&targets, &mut std::io::BufWriter::new(&mut buf));
        let summary = String::from_utf8(buf)?;
        println!("Following images will be deleted:\n{}", summary);
        println!("Now Applying...");
        dyson.delete_images(&targets).await?;
        dyson.notify_result("Apply Succeeded!!", targets).await?;
        println!("Apply Complete!");
        Ok(())
    }

    /// Try to initialize a cleaner
    async fn try_new_cleaner(&self) -> Result<Dyson, Box<dyn std::error::Error>> {
        let conf = DysonConfig::load_path(&self.global_args.config_path)?;
        Ok(Dyson::new(&conf).await?)
    }
}

/// Global arguments
#[derive(clap::Args)]
pub struct GlobalArgs {
    /// Path to config file
    #[arg(
        value_name = "FILE",
        short,
        long = "config",
        global = true,
        default_value = "dyson.yaml"
    )]
    config_path: String,
}

/// List of commands
#[derive(clap::Subcommand)]
pub enum Commands {
    /// Generate a config file
    Init(InitArgs),
    /// Make a deletion plan according to the config
    Plan,
    /// Delete ECR images according to the config
    Apply,
}

/// arguments for init command
#[derive(clap::Args)]
pub struct InitArgs {
    /// Weather to write to stdout
    #[arg(long, default_value = "false")]
    stdout: bool,
}
