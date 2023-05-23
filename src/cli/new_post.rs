use clap::Parser;

#[derive(Parser)]
pub struct NewPostOptions {
    title: String,
}

impl super::Command for NewPostOptions {
    async fn run(self) -> eyre::Result<()> {
        println!("Creating new post with title: {}", self.title);
        Ok(())
    }
}
