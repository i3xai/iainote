# iainote 设计方案

> 为 AI 时代打造的云端笔记系统 — 让 AI 的经验可以被记录、被访问、被复用

---

## 一、项目定位与愿景

**一句话描述：** iainote 是一个双端笔记系统，人类通过 CLI 查询和管理，AI 通过 API 读写笔记，服务端部署在云端，数据归属用户本人。

**核心场景：**
- 用户（人类）用 CLI 记录自己的经验笔记
- AI（Claude、GPT、本地模型等）通过 API 读取上下文相关的笔记来增强回答质量
- AI 在完成对话/任务后将关键结论写入笔记，供后续调用
- 用户为不同 AI 分配不同的 Key，实现多 AI 协作和数据隔离

---

## 二、核心概念

### 2.1 用户体系

| 概念 | 说明 |
|------|------|
| **用户（User）** | 人类用户，通过邮箱注册 |
| **Key** | 用户生成的 API Key，每个 Key 对应一个 AI 身份 |
| **Key Group** | 同一用户的多个 Key 可以组成一个数据池，支持数据迁移和合并 |

### 2.2 数据模型

```
User
  └── Key[]              一个用户可拥有多个 Key
        ├── key_id
        ├── key_hash     （存储 SHA256 哈希）
        ├── name         "我的 Claude"
        ├── created_at
        └── notes[]       该 Key 写入的笔记

Note
  ├── id                  UUID
  ├── user_id
  ├── key_id              写入这条笔记的 Key
  ├── title
  ├── content             Markdown 正文
  ├── tags[]              ["vps", "frp", "linux"]
  ├── visibility          "private" | "shared"   （shared 可被同用户其他 Key 读取）
  ├── version             递增版本号，用于乐观锁
  ├── created_at
  └── updated_at
```

---

## 三、技术选型

### 3.1 语言选择

**推荐：Rust（生产级）**

| 对比维度 | Go | Rust |
|---------|-----|------|
| 性能 | ★★★★★ | ★★★★★ |
| 编译后单二进制 | 需要 CGO | 真正静态单二进制 |
| Web 生态 | Gin/Echo 成熟 | Actix-web/Rust 生态成熟 |
| AI 场景主流度 | 中 | 低 |
| 学习曲线 | 平缓 | 陡峭 |
| 并发模型 | goroutine | tokio async |

**建议：** 核心 API 服务用 Rust，保证高性能和零依赖部署体验；CLI 工具也用 Rust，保证工具链统一。

### 3.2 技术栈

| 组件 | 技术选型 |
|------|---------|
| **API 服务端** | Rust + Actix-web + SQLx |
| **数据库** | PostgreSQL（支持全文搜索） |
| **缓存层** | Redis（Token 计数、速率限制） |
| **CLI 工具** | Rust + clap + reqwest |
| **认证** | Key = SHA256(user_id + secret)，JWT 短期 Token |
| **部署** | 静态二进制，Docker 或直接拉取 |

### 3.3 数据库设计

```sql
-- 用户表
CREATE TABLE users (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    email       VARCHAR(255) UNIQUE NOT NULL,
    password_hash VARCHAR(255) NOT NULL,
    created_at  TIMESTAMPTZ DEFAULT NOW(),
    updated_at  TIMESTAMPTZ DEFAULT NOW()
);

-- Key 表
CREATE TABLE api_keys (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id     UUID REFERENCES users(id) ON DELETE CASCADE,
    name        VARCHAR(100) NOT NULL,        -- "Claude-3.5-Sonnet"
    key_hash    VARCHAR(64) UNIQUE NOT NULL,   -- SHA256(actual_key)
    created_at  TIMESTAMPTZ DEFAULT NOW(),
    revoked     BOOLEAN DEFAULT FALSE
);

-- 笔记表
CREATE TABLE notes (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id     UUID REFERENCES users(id) ON DELETE CASCADE,
    key_id      UUID REFERENCES api_keys(id) ON DELETE SET NULL,
    title       VARCHAR(255) NOT NULL,
    content     TEXT NOT NULL,
    visibility  VARCHAR(20) DEFAULT 'private',
    version     INTEGER DEFAULT 1,
    created_at  TIMESTAMPTZ DEFAULT NOW(),
    updated_at  TIMESTAMPTZ DEFAULT NOW()
);

-- 标签表
CREATE TABLE tags (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id     UUID REFERENCES users(id) ON DELETE CASCADE,
    name        VARCHAR(100) NOT NULL
);

-- 笔记-标签关联表
CREATE TABLE note_tags (
    note_id     UUID REFERENCES notes(id) ON DELETE CASCADE,
    tag_id      UUID REFERENCES tags(id) ON DELETE CASCADE,
    PRIMARY KEY (note_id, tag_id)
);

-- 全文索引（PostgreSQL 内置）
CREATE INDEX idx_notes_content_fts ON notes USING GIN (to_tsvector('english', title || ' ' || content));
CREATE INDEX idx_notes_tags ON note_tags(tag_id);
CREATE INDEX idx_notes_user ON notes(user_id);
```

---

## 四、API 设计

### 4.1 认证方式

```
Authorization: Bearer ia_sk_xxxxxxxxxxxxxxxxxxxx
```

Key 格式：`ia_sk_` + 32字符随机字符串（用户可见一次，之后只存 hash）

### 4.2 接口列表

#### 认证类

| 方法 | 路径 | 说明 |
|------|------|------|
| POST | `/api/v1/auth/register` | 用户注册 |
| POST | `/api/v1/auth/login` | 登录，返回 JWT |
| POST | `/api/v1/auth/keys` | 创建一个新的 Key |
| GET | `/api/v1/auth/keys` | 列出当前用户所有 Key |
| DELETE | `/api/v1/auth/keys/{id}` | 撤销一个 Key |

#### 笔记类

| 方法 | 路径 | 说明 |
|------|------|------|
| GET | `/api/v1/notes` | 列出笔记（支持分页、标签过滤、visibility过滤） |
| POST | `/api/v1/notes` | 创建笔记 |
| GET | `/api/v1/notes/{id}` | 获取单条笔记 |
| PUT | `/api/v1/notes/{id}` | 更新笔记（版本校验） |
| DELETE | `/api/v1/notes/{id}` | 删除笔记 |
| GET | `/api/v1/notes/search?q=` | 模糊检索（全文搜索） |

#### 标签类

| 方法 | 路径 | 说明 |
|------|------|------|
| GET | `/api/v1/tags` | 列出用户所有标签 |
| POST | `/api/v1/tags` | 创建标签 |
| DELETE | `/api/v1/tags/{id}` | 删除标签 |

#### Key 数据管理类

| 方法 | 路径 | 说明 |
|------|------|------|
| POST | `/api/v1/keys/{id}/merge` | 合并两个 Key 的数据 |
| POST | `/api/v1/keys/{id}/transfer` | 将某 Key 的笔记迁移到另一个 Key |

#### AI 专用接口（优化过）

| 方法 | 路径 | 说明 |
|------|------|------|
| GET | `/api/v1/ai/search?q=&tags=&limit=` | AI 检索接口，返回结构化 JSON |
| POST | `/api/v1/ai/ingest` | AI 批量写入笔记（从对话中提取） |

### 4.3 请求/响应示例

**创建笔记：**
```json
POST /api/v1/notes
{
  "title": "FRP 内网穿透配置笔记",
  "content": "## 服务器端\n\n配置文件在 /etc/frp/frps.ini\n\n## 客户端\n\n...",
  "tags": ["frp", "vps", "网络"],
  "visibility": "private"
}

Response:
{
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "title": "FRP 内网穿透配置笔记",
  "version": 1,
  "created_at": "2026-04-14T22:00:00Z"
}
```

**模糊检索：**
```json
GET /api/v1/notes/search?q=FRP+穿透+vps

Response:
{
  "results": [
    {
      "id": "550e8400-e29b-41d4-a716-446655440000",
      "title": "FRP 内网穿透配置笔记",
      "snippet": "...服务器端配置在 <mark>/etc/frp</mark>...",
      "tags": ["frp", "vps"],
      "score": 0.95
    }
  ],
  "total": 1
}
```

---

## 五、CLI 工具设计（iainote CLI）

### 5.1 安装方式

```bash
# 一键安装（Linux/macOS）
curl -fsSL https://iaiaiai.cc/install.sh | bash

# 或用 cargo 安装
cargo install iainote-cli
```

### 5.2 命令列表

```
iainote auth login              # 登录（输入邮箱密码，获取 Key）
iainote auth logout             # 登出
iainote auth key list           # 列出所有 Key
iainote auth key create <name>  # 创建新 Key

iainote note new                # 创建笔记（交互式，支持 Ctrl+C 退出）
iainote note list               # 列出笔记
iainote note get <id>           # 查看单条笔记
iainote note edit <id>          # 编辑笔记
iainote note delete <id>         # 删除笔记

iainote search <关键词>         # 全局模糊检索
iainote tag list                # 列出所有标签
iainote tag create <name>       # 创建标签
iainote tag note add <note-id> <tag>   # 给笔记打标签

iainote sync                    # 同步本地缓存的笔记列表
```

### 5.3 交互式创建笔记

```bash
$ iainote note new
标题: FRP 内网穿透完整指南
标签 (逗号分隔): frp, vps, 网络配置
可见性 [private/shared]: private
正文 (Ctrl+D 结束):
## 一、FRP 简介

FRP 是一个高性能的反向代理工具...

^D
✅ 笔记已创建: 550e8400-e29b-41d4-a716-446655440000
```

---

## 六、AI 接入方式

### 6.1 AI 读取笔记（给 AI 的 System Prompt 片段）

```
你有一个专属笔记系统，通过以下端点访问笔记内容：

搜索笔记：
curl -H "Authorization: Bearer YOUR_KEY" \
  "https://api.iaiaiai.cc/v1/ai/search?q=FRP+vps&limit=5"

读取完整笔记：
curl -H "Authorization: Bearer YOUR_KEY" \
  "https://api.iaiaiai.cc/v1/notes/{id}"

在回答用户问题前，先搜索你的笔记库，看是否有相关内容可以引用。
如果笔记中有相关信息，在回答中引用，格式：[来源: 笔记标题]
```

### 6.2 AI 写入笔记

```bash
curl -X POST "https://api.iaiaiai.cc/v1/ai/ingest" \
  -H "Authorization: Bearer YOUR_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "title": "用户询问 FRP 内网穿透",
    "content": "用户有一台阿里云轻量服务器...",
    "tags": ["frp", "vps", "阿里云"],
    "auto_tag": true
  }'
```

### 6.3 多 AI 数据隔离

- 用户可以为 Claude 申请 Key A，为 GPT 申请 Key B
- 每个 Key 写入的笔记默认私有
- 同用户的 Key 可以设置"数据共享组"，让不同 AI 共享同一知识库
- Key 之间支持数据迁移（将 Key B 的笔记合并到 Key A）

---

## 七、项目结构

```
iainote/
├── src/
│   ├── main.rs              # 入口，Actix-web server
│   ├── auth/                # 认证模块（register, login, key 管理）
│   ├── notes/               # 笔记 CRUD
│   ├── search/              # 全文搜索
│   ├── tags/                # 标签管理
│   ├── ai/                  # AI 专用接口
│   └── db/                  # 数据库连接、迁移
├── migrations/              # SQLx 离线迁移文件
├── cli/                     # CLI 工具源码
│   ├── main.rs
│   └── commands/
├── website/                 # 官网 / Landing Page（纯 HTML/CSS/JS）
│   ├── index.html
│   ├── docs.html
│   ├── download.html
│   ├── styles/
│   │   └── main.css
│   ├── scripts/
│   │   └── main.js
│   └── assets/
├── Cargo.toml
├── Dockerfile
└── docker-compose.yml
```

---

## 八、部署方案

### 8.1 部署到你的服务器（139.224.28.252）

```bash
# 在服务器上
git clone https://github.com/i3xai/iainote.git
cd iainote
docker-compose up -d

# 或直接二进制部署
curl -fsSL https://iaiaiai.cc/iainote-server_latest_amd64.tar.gz | tar xz
./iainote-server --db-url postgres://user:pass@localhost:5432/iainote
```

### 8.2 PostgreSQL 和 Redis 通过 Docker Compose 启动

```yaml
version: '3.8'
services:
  db:
    image: postgres:16-alpine
    environment:
      POSTGRES_DB: iainote
      POSTGRES_USER: iainote
      POSTGRES_PASSWORD: ${DB_PASSWORD}
    volumes:
      - pgdata:/var/lib/postgresql/data
    ports:
      - "5432:5432"

  redis:
    image: redis:7-alpine
    ports:
      - "6379:6379"

  api:
    build: .
    ports:
      - "8080:8080"
    depends_on:
      - db
      - redis
    environment:
      DATABASE_URL: postgres://iainote:${DB_PASSWORD}@db:5432/iainote
      REDIS_URL: redis://redis:6379

volumes:
  pgdata:
```

---

## 九、官方网站 / Landing Page

**设计风格：Neo-Terminal Brutalism（2026年 AI 工具最流行风格）**

参考：Vercel、Linear、Raycast 的视觉语言 + 终端黑客美学

### 9.1 视觉规范

| 元素 | 规范 |
|------|------|
| **主色调** | `#0a0a0a`（纯黑背景） |
| **强调色** | `#00ff88`（霓虹绿，terminal 感） |
| **辅助色** | `#00d4ff`（电光蓝）、`#ff6b35`（警示橙） |
| **边框** | 2px solid 纯色，粗犷线条感 |
| **圆角** | 4px（几乎不圆，直角为主） |
| **字体** | JetBrains Mono（正文/代码）、Inter（UI 标签） |
| **动效** | 打字机效果、终端光标闪烁、滚动触发的渐入 |
| **背景** | 暗色调 + 网格线 + 噪点纹理 |

### 9.2 页面结构

```
┌─────────────────────────────────────────────────┐
│  Header: Logo + Nav + GitHub + Start 按钮       │
├─────────────────────────────────────────────────┤
│  Hero Section                                   │
│  ├─ 动态终端模拟窗口（展示 CLI 操作动画）        │
│  ├─ 标题：AI-Native Notes for AI & Humans        │
│  └─ 副标题 + 双 CTA 按钮（Get Started / GitHub） │
├─────────────────────────────────────────────────┤
│  Features Grid（3 列）                          │
│  ├─ AI-First API                                │
│  ├─ Terminal-First CLI                          │
│  └─ Key-Level Data Isolation                    │
├─────────────────────────────────────────────────┤
│  Terminal Demo Section                          │
│  ├─ 安装命令 curl 安装                           │
│  └─ 核心命令演示（动态打字效果）                  │
├─────────────────────────────────────────────────┤
│  How It Works（3 步骤）                         │
│  ├─ 1. 注册并创建 Key                           │
│  ├─ 2. AI 通过 API 读写笔记                     │
│  └─ 3. 人类通过 CLI 查询管理                     │
├─────────────────────────────────────────────────┤
│  Code Example Section                           │
│  ├─ 左侧：API 请求示例（curl/JS）                │
│  └─ 右侧：响应示例（JSON）                       │
├─────────────────────────────────────────────────┤
│  Open Source Banner（强调开源）                  │
├─────────────────────────────────────────────────┤
│  Footer                                         │
│  ├─ GitHub 链接                                 │
│  ├─ 文档链接                                    │
│  └─ 版权信息                                    │
└─────────────────────────────────────────────────┘
```

### 9.3 页面文件结构

```
website/
├── index.html              # 主页入口
├── styles/
│   └── main.css            # 所有样式（纯 CSS，无框架依赖）
├── scripts/
│   └── main.js             # 动效逻辑
├── assets/
│   └── terminal-demo.gif   # 终端演示（可后续生成）
├── docs.html               # 文档页
└── download.html           # 下载页面
```

### 9.4 Hero 动态终端窗口（JS 模拟）

```javascript
// 终端打字机效果
const commands = [
  '$ iainote auth login',
  '> 登录成功，欢迎回来！',
  '$ iainote note new',
  '> 标题: FRP 内网穿透笔记',
  '> 标签: frp, vps, 网络',
  '$ iainote search FRP',
  '> 找到 3 条笔记，耗时 12ms',
  '$ iainote note get 550e8400...',
  '> ## FRP 内网穿透配置笔记\n> ...',
];

// 循环播放，自动进入下一个命令
// 光标闪烁：| → ▋
```

### 9.5 主页技术实现

- **纯 HTML/CSS/JS**（零依赖，2026 最干净的前端方式）
- 无需构建工具，直接 `python3 -m http.server` 即可本地预览
- 也可以用任意静态托管：GitHub Pages / Vercel / Cloudflare Pages / 部署到 139.224.28.252

### 9.6 SEO 和元信息

```html
<title>iainote - AI 时代的云端笔记系统</title>
<meta name="description" content="为 AI 打造的云端笔记系统。CLI 管理、API 读写、数据隔离、模糊检索。开源免费。">
<meta property="og:title" content="iainote - AI-Native Cloud Notes">
<meta property="og:description" content="记录 AI 的经验，让 AI 和人类共同访问。开源项目。">
<meta property="og:type" content="website">
<link rel="icon" type="image/svg+xml" href="/favicon.svg">
```

---

## 九（续）、后续开发计划

### Phase 1（官网 + MVP 并行）
- [ ] Landing Page（官网主页，纯 HTML/CSS/JS，Neo-Terminal 风格）
- [ ] 下载页面（各平台 CLI 客户端下载）
- [ ] 用户注册/登录
- [ ] Key 的创建和管理
- [ ] 笔记 CRUD + 标签系统

### Phase 2（完善）
- [ ] CLI 工具完整功能
- [ ] PostgreSQL 全文搜索 + 模糊检索
- [ ] Key 数据迁移和合并
- [ ] AI 专用搜索接口（结构化 JSON 返回）
- [ ] 笔记版本历史

### Phase 3（生态）
- [ ] Web 管理界面
- [ ] 浏览器插件（划词保存到 iainote）
- [ ] API 调用量统计
- [ ] 公开笔记广场（用户可选择分享笔记）

---

## 十、核心亮点（差异化）

| 维度 | 普通笔记（Notion/飞书） | iainote |
|------|----------------------|---------|
| AI 可读性 | ❌ 弱（富文本，AI 难以解析） | ✅ 原生 JSON/Markdown |
| AI 可写 | ❌ 不支持 | ✅ API 原生支持 |
| 多 AI 隔离 | ❌ 不支持 | ✅ Key 级别数据隔离 |
| CLI 优先 | ❌ 不支持 | ✅ 极客体验 |
| 数据迁移 | ❌ 导出困难 | ✅ Key 一键迁移 |

---

*文档版本：v1.0 | 创建时间：2026-04-14 | 作者：i3xai & Claude*
