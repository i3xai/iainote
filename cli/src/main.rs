use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "iainote")]
#[command(version = "0.1.0")]
#[command(about = "AI-Native Notes CLI", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Authenticate with iainote server
    Auth {
        #[command(subcommand)]
        command: AuthCommands,
    },
    /// Manage notes
    Note {
        #[command(subcommand)]
        command: NoteCommands,
    },
    /// Search notes
    Search {
        query: Vec<String>,
    },
    /// Manage tags
    Tag {
        #[command(subcommand)]
        command: TagCommands,
    },
}

#[derive(Subcommand)]
enum AuthCommands {
    /// Login with email and password
    Login,
    /// Logout and clear credentials
    Logout,
    /// Manage API keys
    Key {
        #[command(subcommand)]
        command: KeyCommands,
    },
}

#[derive(Subcommand)]
enum KeyCommands {
    /// List all API keys
    List,
    /// Create a new API key
    Create { name: String },
}

#[derive(Subcommand)]
enum NoteCommands {
    /// Create a new note
    New,
    /// List all notes
    List,
    /// Get a note by ID
    Get { id: String },
    /// Edit a note
    Edit { id: String },
    /// Delete a note
    Delete { id: String },
}

#[derive(Subcommand)]
enum TagCommands {
    /// List all tags
    List,
    /// Create a new tag
    Create { name: String },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Auth { command } => match command {
            AuthCommands::Login => {
                println!("🔐 登录 iainote...");
                println!("请输入邮箱: ");
                // TODO: interactive login
            }
            AuthCommands::Logout => {
                println!("👋 已退出登录");
            }
            AuthCommands::Key { command } => match command {
                KeyCommands::List => {
                    println!("📋 API Keys:");
                }
                KeyCommands::Create { name } => {
                    println!("🔑 创建 Key: {}", name);
                }
            },
        },
        Commands::Note { command } => match command {
            NoteCommands::New => {
                println!("📝 创建新笔记...");
            }
            NoteCommands::List => {
                println!("📚 笔记列表:");
            }
            NoteCommands::Get { id } => {
                println!("📄 读取笔记: {}", id);
            }
            NoteCommands::Edit { id } => {
                println!("✏️  编辑笔记: {}", id);
            }
            NoteCommands::Delete { id } => {
                println!("🗑️  删除笔记: {}", id);
            }
        },
        Commands::Search { query } => {
            let q = query.join(" ");
            println!("🔍 搜索: {}", q);
        }
        Commands::Tag { command } => match command {
            TagCommands::List => {
                println!("🏷️  标签列表:");
            }
            TagCommands::Create { name } => {
                println!("🏷️  创建标签: {}", name);
            }
        },
    }

    Ok(())
}
