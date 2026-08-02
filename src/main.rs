#![cfg(windows)]

use clap::Parser;
use regex::Regex;
use std::env;
use std::fs::{self, File};
use std::io::{self, BufRead, BufReader};
use std::path::{Component, Path, PathBuf};

#[derive(Parser, Debug)]
#[command(
    name = "md2proj",
    version = "1.0",
    about = "从 Markdown (AI 输出) 文件中提取代码块并自动生成对应的项目文件。"
)]
struct Args {
    #[arg(
        default_value = "ai_output.md",
        help = "目标 Markdown 文件的路径 (默认: ai_output.md)"
    )]
    file: String,
}

fn main() {
    let args = Args::parse();
    if let Err(e) = parse_markdown_to_files(&args.file) {
        eprintln!("❌ 发生错误: {}", e);
    }
}

fn parse_markdown_to_files(md_file_path: &str) -> io::Result<()> {
    let md_path = Path::new(md_file_path);
    if !md_path.is_file() {
        println!("❌ 错误：找不到文件或路径无效: {}", md_file_path);
        return Ok(());
    }

    let re_explicit = Regex::new(r"(?i)(?:file|文件)\s*[:：]\s*[*`]*([^`\s*()]+)").unwrap();
    let re_list_backtick =
        Regex::new(r"^(?:#+\s*|\d+\.\s*|[-*]\s*).*?`([^`\s]+\.[a-zA-Z0-9]+)`").unwrap();
    let re_heading_bare = Regex::new(r"^#+\s*[*`]*([a-zA-Z0-9_/-]+\.[a-zA-Z0-9]+)").unwrap();

    // 新增：行内提取规则与意图验证规则
    let re_inline = Regex::new(r"`([^`\s]+\.[a-zA-Z0-9]+)`").unwrap();
    let re_action = Regex::new(r"(?i)(file|文件|create|创建|写入)").unwrap();

    let base_dir = env::current_dir()?;

    let mut candidate_path: Option<String> = None;
    let mut current_target_path: Option<PathBuf> = None;
    let mut in_code_block = false;
    let mut code_lines = String::new();

    // 新增：维护一个历史已知路径列表，用于短路径解析回溯
    let mut known_paths: Vec<String> = Vec::new();

    let file = File::open(md_path)?;
    let mut reader = BufReader::new(file);
    let mut line = String::new();

    while reader.read_line(&mut line)? > 0 {
        let trimmed_line = line.trim();

        if !in_code_block {
            let mut found_path = None;

            // 强匹配规则
            if let Some(cap) = re_explicit.captures(trimmed_line) {
                found_path = Some(cap[1].to_string());
            } else if let Some(cap) = re_list_backtick.captures(trimmed_line) {
                found_path = Some(cap[1].to_string());
            } else if let Some(cap) = re_heading_bare.captures(trimmed_line) {
                found_path = Some(cap[1].to_string());
            }

            if let Some(raw_path) = found_path {
                candidate_path = Some(raw_path.clone());
                if !known_paths.contains(&raw_path) {
                    known_paths.push(raw_path.clone());
                }
                println!("🔍 嗅探到候选文件: {}", raw_path);
            } else {
                // 强规则没命中，尝试从普通段落中提取代码块之前提及的文件名
                let mut matched_inline = false;

                for cap in re_inline.captures_iter(trimmed_line) {
                    let short_path = cap[1].to_string();
                    let mut resolved_path = None;

                    // 1. 尝试用短文件名（如 build.yml）去匹配已知路径列表中尾部长文件名
                    for p in &known_paths {
                        if p == &short_path
                            || p.ends_with(&format!("/{}", short_path))
                            || p.ends_with(&format!("\\{}", short_path))
                        {
                            resolved_path = Some(p.clone());
                            break;
                        }
                    }

                    if let Some(res) = resolved_path {
                        candidate_path = Some(res.clone());
                        matched_inline = true;
                    } else if re_action.is_match(trimmed_line) {
                        // 2. 如果之前没出现过，但当前行包含“文件/创建/写入”等动作词，予以信任
                        candidate_path = Some(short_path.clone());
                        if !known_paths.contains(&short_path) {
                            known_paths.push(short_path.clone());
                        }
                        matched_inline = true;
                    }
                }

                if matched_inline {
                    if let Some(ref p) = candidate_path {
                        println!("🔍 嗅探到候选文件 (段落解析): {}", p);
                    }
                }
            }
        }

        // 代码块开始/结束处理
        if trimmed_line.starts_with("```") {
            if !in_code_block {
                in_code_block = true;
                code_lines.clear();

                if let Some(raw_path) = candidate_path.take() {
                    let safe_rel_path = raw_path.trim_start_matches(|c| c == '/' || c == '\\');
                    let rel_path = Path::new(safe_rel_path);

                    let mut is_safe = true;
                    for component in rel_path.components() {
                        if matches!(
                            component,
                            Component::ParentDir | Component::RootDir | Component::Prefix(_)
                        ) {
                            is_safe = false;
                            break;
                        }
                    }

                    if is_safe {
                        let target_path = base_dir.join(rel_path);
                        if target_path.file_stem().and_then(|n| n.to_str()) == Some("4wh761km1h") {
                            println!("🚫 拦截：已跳过受保护的文件 {}", raw_path);
                        } else {
                            current_target_path = Some(target_path);
                            println!("\n📄 关联成功，准备写入目标文件: {}", raw_path);
                        }
                    } else {
                        println!("⚠️ 警告：检测到非法越权路径，已跳过保护: {}", raw_path);
                    }
                }
            } else {
                // 代码块结束
                in_code_block = false;
                if let Some(target) = current_target_path.take() {
                    write_file(&target, &code_lines, &base_dir);
                }
            }
            line.clear();
            continue;
        }

        if in_code_block && current_target_path.is_some() {
            code_lines.push_str(&line);
        }

        line.clear();
    }

    if in_code_block {
        if let Some(target) = current_target_path.take() {
            println!("\n⚠️ 警告：检测到未闭合的代码块，正在强制保存已有内容...");
            write_file(&target, &code_lines, &base_dir);
        }
    }

    Ok(())
}

fn write_file(target_path: &Path, code_lines: &str, base_dir: &Path) {
    if let Some(parent) = target_path.parent() {
        if let Err(e) = fs::create_dir_all(parent) {
            println!("  └─ ❌ 创建目录失败 {}: {}", parent.display(), e);
            return;
        }
    }

    let content = code_lines.replace('\u{00a0}', " ");

    match fs::write(target_path, content) {
        Ok(_) => {
            let rel_path = target_path.strip_prefix(base_dir).unwrap_or(target_path);
            println!("  └─ ✅ 成功写入/覆盖: {}", rel_path.display());
        }
        Err(e) => {
            let name = target_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");
            println!("  └─ ❌ 写入文件失败 {}: {}", name, e);
        }
    }
}
