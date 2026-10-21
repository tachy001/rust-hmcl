//! Conservative launch failure clues. Exit codes alone never identify a cause.
use serde::Serialize;

#[derive(Clone, Serialize)]
pub struct Clue {
    pub title: String,
    pub evidence: String,
    pub suggestion: String,
    pub action: String,
}

pub fn launch_clues(error: &str, logs: &[String]) -> Vec<Clue> {
    let rules: &[(&str, &[&str], &str, &str)] = &[
        (
            "加载器安装产物需要修复",
            &["Forge 安装产物缺失或损坏"],
            "校验该实例，在独立副本中重新执行该加载器的官方安装器以重建依赖；需要 Java。",
            "repair",
        ),
        (
            "Java 版本或路径需要检查",
            &[
                "UnsupportedClassVersionError",
                "低于游戏需要的 Java",
                "无法运行所选 Java",
                "请在 Java 管理页面下载或设置路径",
                "旧版游戏请使用 Java 8",
                "不适配当前启动器",
            ],
            "核对该实例需要的 Java 版本、架构与单独指定的路径。",
            "java",
        ),
        (
            "Java 无法预留所需内存",
            &[
                "Could not reserve enough space",
                "There is insufficient memory for the Java Runtime Environment",
                "Native memory allocation (mmap) failed",
            ],
            "关闭不需要的程序，检查实例内存设置，为系统保留可用内存。",
            "settings",
        ),
        (
            "Java 堆内存耗尽",
            &[
                "OutOfMemoryError: Java heap space",
                "OutOfMemoryError: GC overhead limit exceeded",
            ],
            "检查实例内存与模组需求；调整前确认系统还有足够可用内存。",
            "settings",
        ),
        (
            "游戏主类未能加载",
            &[
                "Could not find or load main class",
                "ClassNotFoundException: net.minecraft.client.main.Main",
            ],
            "可先校验并补全游戏文件；如果是自定义加载器，请同时检查其安装是否完整。",
            "repair",
        ),
        (
            "模组依赖或版本存在冲突",
            &[
                "Incompatible mods found!",
                "Mod resolution encountered an incompatible mod set",
            ],
            "按日志列出的要求检查模组与依赖版本，可在模组管理中逐项停用排查。",
            "mods",
        ),
    ];
    let mut clues = vec![];
    for (title, patterns, suggestion, action) in rules {
        let evidence = std::iter::once(error)
            .chain(logs.iter().map(String::as_str))
            .flat_map(str::lines)
            .find(|line| patterns.iter().any(|pattern| line.contains(pattern)));
        if let Some(evidence) = evidence {
            clues.push(Clue {
                title: (*title).into(),
                evidence: evidence.chars().take(400).collect(),
                suggestion: (*suggestion).into(),
                action: (*action).into(),
            });
        }
    }
    clues
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exit_codes_and_unrelated_exceptions_do_not_invent_a_cause() {
        assert!(launch_clues("游戏异常退出 (exit code: 1)", &[]).is_empty());
        assert!(launch_clues("ClassNotFoundException: my.mod.Custom", &[]).is_empty());
    }
    #[test]
    fn distinguishes_heap_exhaustion_from_failed_memory_reservation() {
        let heap = launch_clues(
            "failed",
            &["java.lang.OutOfMemoryError: Java heap space".into()],
        );
        assert_eq!(heap[0].title, "Java 堆内存耗尽");
        let native = launch_clues(
            "Could not reserve enough space for 2097152KB object heap",
            &[],
        );
        assert_eq!(native[0].title, "Java 无法预留所需内存");
        assert_eq!(native[0].action, "settings");
    }
    #[test]
    fn captures_actual_evidence_and_multiple_independent_clues() {
        assert_eq!(
            launch_clues("Forge 安装产物缺失或损坏，请在游戏库中校验修复", &[])[0].action,
            "repair"
        );
        let line = "[main/ERROR] Incompatible mods found!";
        let clues = launch_clues("无法运行所选 Java，请检查路径", &[line.into()]);
        assert_eq!(clues.len(), 2);
        assert_eq!(clues[1].evidence, line);
        assert_eq!(clues[1].action, "mods");
    }
}
