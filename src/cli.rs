use std::path::Path;

use crate::config::DysonConfig;
use crate::dyson::Dyson;
use crate::summary::print_summary;

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
            Commands::Plan => self.run_plan_command(&mut std::io::stdout()).await,
            Commands::Apply => self.run_apply_command(&mut std::io::stdout()).await,
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
    async fn run_plan_command(
        &self,
        output: &mut impl std::io::Write,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let dyson = self.try_new_cleaner().await?;
        let targets = dyson.list_target_images().await?;
        println!("Plan Result:");
        print_summary(&targets, output);
        Ok(())
    }

    /// Run the apply command
    async fn run_apply_command(
        &self,
        output: &mut impl std::io::Write,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let dyson = self.try_new_cleaner().await?;
        let targets = dyson.list_target_images().await?;
        println!("Delete following images:");
        print_summary(&targets, output);
        dyson.delete_images(targets).await?;
        println!("Delete Complete!");
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
