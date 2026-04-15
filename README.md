# iainote

> AI 时代的云端笔记系统 — 让 AI 的经验可以被记录、被访问、被复用

## 项目组成

```
iainote/
├── server/     # Rust API 服务端 (Actix-web + SQLx + PostgreSQL)
├── cli/        # Rust CLI 工具 (clap)
└── website/   # 官网 (静态 HTML/CSS/JS)
```

## 快速开始

### 服务端

```bash
cd server
cp .env.example .env
# 编辑 .env 设置数据库连接
cargo run
```

### CLI

```bash
cargo install --path cli
iainote auth login
```

## 技术栈

- **服务端**: Rust + Actix-web + SQLx + PostgreSQL + Redis
- **CLI**: Rust + clap + reqwest
- **数据库**: PostgreSQL 16 (全文搜索)
- **缓存**: Redis 7

## 许可证

MIT OR Apache-2.0
