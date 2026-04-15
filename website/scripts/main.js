// ========================================
// iainote — Landing Page Scripts
// ========================================

// ---- Terminal Typewriter Effect ----
const TERMINAL_LINES = [
  { type: 'cmd', text: '$ iainote auth login' },
  { type: 'out', text: '> 登录成功，欢迎回来！' },
  { type: 'cmd', text: '$ iainote note new' },
  { type: 'out', text: '> 标题: FRP 内网穿透完整指南' },
  { type: 'out', text: '> 标签: frp, vps, 网络配置' },
  { type: 'out', text: '> 正文: ## 服务器配置' },
  { type: 'out', text: '> 服务器地址: 139.224.28.252' },
  { type: 'cmd', text: '$ iainote search FRP 内网穿透' },
  { type: 'out', text: '> 找到 1 条笔记，耗时 8ms ✓' },
  { type: 'cmd', text: '$ iainote note get 550e8400-e29b-41d4' },
  { type: 'out', text: '> ## FRP 内网穿透完整指南' },
  { type: 'out', text: '> 标签: [frp, vps, 网络配置]' },
  { type: 'out', text: '> ## 服务器端\n> bind_port = 7000' },
];

let termIndex = 0;
let charIndex = 0;
let currentLineEl = null;
let isTyping = false;
let timeoutId = null;

function sleep(ms) {
  return new Promise(resolve => { timeoutId = setTimeout(resolve, ms); });
}

async function typeTerminal() {
  const output = document.getElementById('terminal-output');
  if (!output) return;

  // Keep only first line (always visible)
  const firstLine = output.querySelector('.terminal-line');
  output.innerHTML = '';
  if (firstLine) output.appendChild(firstLine);

  termIndex = 0;
  charIndex = 0;
  currentLineEl = null;
  isTyping = true;
  await runTerminal();
}

async function runTerminal() {
  const output = document.getElementById('terminal-output');
  if (!output || !isTyping) return;

  if (termIndex >= TERMINAL_LINES.length) {
    await sleep(2000);
    termIndex = 0;
    output.innerHTML = '';
  }

  const line = TERMINAL_LINES[termIndex];

  if (!currentLineEl) {
    currentLineEl = document.createElement('div');
    currentLineEl.className = `terminal-line line-${line.type}`;
    output.appendChild(currentLineEl);
  }

  if (charIndex < line.text.length) {
    currentLineEl.textContent += line.text[charIndex];
    charIndex++;
    await sleep(35 + Math.random() * 20);
    await runTerminal();
  } else {
    await sleep(600);
    termIndex++;
    charIndex = 0;
    currentLineEl = null;
    await runTerminal();
  }
}

// Cursor blink
function blinkCursor() {
  const cursor = document.getElementById('cursor');
  if (!cursor) return;
  setInterval(() => {
    cursor.textContent = cursor.textContent === '|' ? '▋' : '|';
  }, 530);
}

// ---- Scroll Animations ----
function initScrollAnimations() {
  const observer = new IntersectionObserver((entries) => {
    entries.forEach(entry => {
      if (entry.isIntersecting) {
        entry.target.classList.add('fade-in-up');
        observer.unobserve(entry.target);
      }
    });
  }, { threshold: 0.1, rootMargin: '0px 0px -40px 0px' });

  document.querySelectorAll('.feature-card, .step, .code-panel').forEach(el => {
    el.style.opacity = '0';
    observer.observe(el);
  });
}

// ---- Copy Install Command ----
function copyInstall() {
  const el = document.getElementById('install-cmd');
  if (!el) return;
  const text = el.textContent.trim();
  navigator.clipboard.writeText(text).then(() => {
    const btn = el.parentElement.querySelector('.copy-btn');
    const original = btn.innerHTML;
    btn.textContent = '已复制!';
    setTimeout(() => { btn.innerHTML = original; }, 1500);
  }).catch(() => {
    // Fallback
    const ta = document.createElement('textarea');
    ta.value = text;
    document.body.appendChild(ta);
    ta.select();
    document.execCommand('copy');
    document.body.removeChild(ta);
    const btn = el.parentElement.querySelector('.copy-btn');
    btn.textContent = '已复制!';
    setTimeout(() => { btn.innerHTML = original; }, 1500);
  });
}

// ---- Header scroll effect ----
function initHeaderScroll() {
  const header = document.querySelector('.header');
  let lastY = 0;
  window.addEventListener('scroll', () => {
    const y = window.scrollY;
    if (y > 100) {
      header.style.borderBottomColor = 'var(--green)';
      header.style.boxShadow = '0 1px 20px rgba(0,255,136,0.08)';
    } else {
      header.style.borderBottomColor = 'var(--border)';
      header.style.boxShadow = 'none';
    }
    lastY = y;
  });
}

// ---- Init ----
document.addEventListener('DOMContentLoaded', () => {
  typeTerminal();
  blinkCursor();
  initScrollAnimations();
  initHeaderScroll();

  // Stagger feature cards
  document.querySelectorAll('.feature-card').forEach((card, i) => {
    card.style.transitionDelay = `${i * 60}ms`;
  });
});
