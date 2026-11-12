import { useCallback, useEffect, useRef, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import {
  addSkill,
  addSkillRepo,
  archiveSkill,
  connectRemote,
  createLink,
  deleteArchived,
  formatHome,
  mergeCopies,
  removeLink,
  removeOrphans,
  replaceWithLink,
  restoreSkill,
  revealAgent,
  revealSkill,
  scanLibrary,
  setCentralRoot,
  syncNow,
  syncStatus,
  type ScanResult,
  type SyncStatus,
} from "./api";
import { Icon, type IconName } from "./icons";
import { applyTheme, getTheme, type Theme } from "./theme";
import { getAutoSync, setAutoSync as saveAutoSync, type AutoSyncSetting } from "./autoSync";
import logo from "./assets/logo.png";
import { Archive, Detail, Library, Settings, SyncView, Targets } from "./views";
import "./styles.css";

type View = "library" | "targets" | "archive" | "sync" | "settings";

interface SheetState {
  title: string;
  desc: string;
  warn?: string;
  confirmLabel: string;
  inputPlaceholder?: string;
  altLabel?: string;
  onAlt?: () => void;
  onConfirm: (input: string) => void | Promise<unknown>;
}

const NAV: { id: View; label: string; icon: IconName; tint: string }[] = [
  { id: "library", label: "技能库", icon: "stack", tint: "blue" },
  { id: "targets", label: "目标 Agent", icon: "branch", tint: "purple" },
  { id: "archive", label: "归档", icon: "archive", tint: "gray" },
  { id: "sync", label: "同步", icon: "sync", tint: "green" },
  { id: "settings", label: "设置", icon: "gear", tint: "gray" },
];

export default function App() {
  const [data, setData] = useState<ScanResult | null>(null);
  const [sync, setSync] = useState<SyncStatus | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [view, setView] = useState<View>("library");
  const [detail, setDetail] = useState<string | null>(null);
  const [toast, setToast] = useState<string | null>(null);
  const [sheet, setSheet] = useState<SheetState | null>(null);
  const [sheetInput, setSheetInput] = useState("");
  const [theme, setTheme] = useState<Theme>(getTheme);
  const [autoSync, setAutoSyncState] = useState<AutoSyncSetting>(getAutoSync);
  const [tidying, setTidying] = useState(false);
  const toastTimer = useRef<number | undefined>(undefined);
  const syncingRef = useRef(false);

  const showToast = useCallback((message: string) => {
    setToast(message);
    window.clearTimeout(toastTimer.current);
    toastTimer.current = window.setTimeout(() => setToast(null), 2600);
  }, []);

  useEffect(() => () => window.clearTimeout(toastTimer.current), []);

  const refresh = useCallback(async () => {
    try {
      const [next, nextSync] = await Promise.all([
        scanLibrary(),
        syncStatus().catch(() => null),
      ]);
      setData(next);
      if (nextSync) setSync(nextSync);
      setLoadError(null);
    } catch (error) {
      setLoadError(String(error));
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  useEffect(() => {
    if (!sheet) return;
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") setSheet(null);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [sheet]);

  const apply = useCallback(
    async (action: () => Promise<ScanResult>, done: string) => {
      showToast("处理中…");
      try {
        setData(await action());
        setLoadError(null);
        showToast(done);
        return true;
      } catch (error) {
        showToast(String(error));
        return false;
      }
    },
    [showToast],
  );

  const openAgent = async (agent: string) => {
    try {
      await revealAgent(agent);
    } catch (error) {
      showToast(String(error));
    }
  };

  const connectSync = async (remote: string, username: string, token: string) => {
    if (syncingRef.current) {
      showToast("正在同步，请稍候再连接");
      return false;
    }
    syncingRef.current = true;
    showToast("正在连接…");
    try {
      setSync(await connectRemote(remote, username, token));
      showToast("已连接远端仓库");
      return true;
    } catch (error) {
      showToast(String(error));
      return false;
    } finally {
      syncingRef.current = false;
    }
  };

  const runSync = useCallback(
    async (silent = false) => {
      if (syncingRef.current) return false;
      syncingRef.current = true;
      if (!silent) showToast("正在同步…");
      try {
        setSync(await syncNow());
        setData(await scanLibrary());
        if (!silent) showToast("同步完成");
        return true;
      } catch (error) {
        showToast(String(error));
        void syncStatus().then(setSync).catch(() => {});
        return false;
      } finally {
        syncingRef.current = false;
      }
    },
    [showToast],
  );

  const changeAutoSync = (next: AutoSyncSetting) => {
    setAutoSyncState(next);
    saveAutoSync(next);
  };

  useEffect(() => {
    if (!autoSync.enabled || !sync?.remote) return;
    const timer = window.setInterval(() => void runSync(true), autoSync.minutes * 60_000);
    return () => window.clearInterval(timer);
  }, [autoSync, sync?.remote, runSync]);

  const current = data?.skills.find((skill) => skill.name === detail) ?? null;
  const totalFiles = data?.skills.reduce((sum, skill) => sum + skill.files, 0) ?? 0;
  const orphanCount = data?.orphans.length ?? 0;

  const mergeAll = (skills: string[]) => {
    setSheet({
      title: `合并 ${skills.length} 个技能的副本？`,
      desc: `${skills.join("、")} 在多个 Agent 目录中是独立副本。合并后这些目录会改为指向中央库的链接。`,
      warn: "副本会移到回收站，然后创建指向中央库的链接。中央库内容不受影响。",
      confirmLabel: "合并为链接",
      onConfirm: async () => {
        showToast("处理中…");
        try {
          let latest: ScanResult | null = null;
          for (const skill of skills) {
            latest = await mergeCopies(skill);
          }
          if (latest) setData(latest);
          showToast(`已合并 ${skills.length} 个技能的副本`);
        } catch (error) {
          showToast(String(error));
        }
      },
    });
  };

  const tidyAll = () => {
    if (!data || tidying) return;
    const strays = data.strays;
    const strayNames = [...new Set(strays.map((item) => item.name))];
    const copyNames = data.skills
      .filter((skill) => Object.values(skill.agents).includes("copy"))
      .map((skill) => skill.name);
    const names = [...new Set([...strayNames, ...copyNames])];
    if (names.length === 0) {
      showToast("无需整理：技能都在中央库，Agent 目录只有链接");
      return;
    }
    const parts: string[] = [];
    if (strayNames.length > 0) parts.push(`收集 ${strayNames.length} 个未入库技能（${strayNames.join("、")}）`);
    if (copyNames.length > 0) parts.push(`合并 ${copyNames.length} 个技能的独立副本（${copyNames.join("、")}）`);
    setSheet({
      title: `一键整理 ${names.length} 个技能？`,
      desc: `${parts.join("；")}。整理后 Agent 目录只保留指向中央库的链接。`,
      warn: "Agent 里的原件与副本会移到回收站，可恢复；同名技能以先入库的版本为准。",
      confirmLabel: "开始整理",
      onConfirm: async () => {
        setTidying(true);
        const total = strays.length + copyNames.length;
        let step = 0;
        let latest: ScanResult = data;
        try {
          for (const stray of strays) {
            step += 1;
            showToast(`整理中… ${step}/${total}`);
            if (!latest.skills.some((skill) => skill.name === stray.name)) {
              latest = await addSkill(stray.path);
            }
            latest = await replaceWithLink(stray.name, stray.agent);
          }
          const copies = latest.skills
            .filter((skill) => Object.values(skill.agents).includes("copy"))
            .map((skill) => skill.name);
          for (const name of copies) {
            step += 1;
            showToast(`整理中… ${step}/${total}`);
            latest = await mergeCopies(name);
          }
          setData(latest);
          setLoadError(null);
          showToast(`整理完成：入库 ${strayNames.length} 个，合并副本 ${copies.length} 个`);
        } catch (error) {
          setData(latest);
          showToast(`整理中断：${String(error)}`);
        } finally {
          setTidying(false);
        }
      },
    });
  };

  const replaceCopy = (skill: string, agent: string) => {
    const agentName = data?.agents.find((item) => item.id === agent)?.name ?? agent;
    setSheet({
      title: "替换为链接？",
      desc: `${agentName} 中的 ${skill} 会从独立副本改为指向中央库的链接。之后修改中央库就会同步到这里。`,
      warn: "副本会移到回收站，然后创建链接。",
      confirmLabel: "替换为链接",
      onConfirm: () => apply(() => replaceWithLink(skill, agent), "已替换为链接"),
    });
  };

  const removeAgentLink = (skill: string, agent: string) => {
    const agentName = data?.agents.find((item) => item.id === agent)?.name ?? agent;
    setSheet({
      title: "取消链接？",
      desc: `${agentName} 中的 ${skill} 链接会被移除，中央库技能不受影响。`,
      confirmLabel: "取消链接",
      onConfirm: () => apply(() => removeLink(skill, agent), "已取消链接"),
    });
  };

  const linkAllAgents = async (skill: string, agents: string[]) => {
    if (agents.length === 0) {
      showToast("没有需要建立链接的 Agent");
      return;
    }
    showToast("处理中…");
    let latest: ScanResult | null = null;
    try {
      for (const agent of agents) latest = await createLink(skill, agent);
      if (latest) setData(latest);
      showToast(`已链接 ${agents.length} 个 Agent`);
    } catch (error) {
      if (latest) setData(latest);
      showToast(String(error));
    }
  };

  const unlinkAllAgents = (skill: string, agents: string[]) => {
    if (agents.length === 0) {
      showToast("没有可取消的链接");
      return;
    }
    setSheet({
      title: `取消 ${agents.length} 个链接？`,
      desc: `${skill} 在这 ${agents.length} 个 Agent 目录中的链接会被移除，中央库技能不受影响。`,
      confirmLabel: "全部取消链接",
      onConfirm: async () => {
        showToast("取消链接中… 0/" + agents.length);
        let latest: ScanResult | null = null;
        try {
          let step = 0;
          for (const agent of agents) {
            step += 1;
            showToast(`取消链接中… ${step}/${agents.length}`);
            latest = await removeLink(skill, agent);
          }
          if (latest) setData(latest);
          showToast(`已取消 ${agents.length} 个链接`);
        } catch (error) {
          if (latest) setData(latest);
          showToast(String(error));
        }
      },
    });
  };

  const removeOrphanLinks = (agent: string, count: number) => {
    const agentName = data?.agents.find((item) => item.id === agent)?.name ?? agent;
    setSheet({
      title: `移除 ${count} 个链接？`,
      desc: `${agentName} 中指向中央库之外的链接会被删除。中央库里的技能文件不受影响。`,
      confirmLabel: "移除链接",
      onConfirm: () => apply(() => removeOrphans(agent), `已移除 ${count} 个链接`),
    });
  };

  const copyPath = async (skill: string) => {
    if (!data) return;
    const text = `${data.centralRoot}\\${skill}`;
    try {
      await navigator.clipboard.writeText(text);
    } catch {
      const field = document.createElement("textarea");
      field.value = text;
      document.body.appendChild(field);
      field.select();
      document.execCommand("copy");
      field.remove();
    }
    showToast("已复制路径");
  };

  const changeRoot = async () => {
    const picked = await open({
      directory: true,
      multiple: false,
      title: "选择中央库目录",
    });
    if (typeof picked === "string") {
      await apply(() => setCentralRoot(picked), "中央库位置已更新");
    }
  };

  const addFromFolder = async () => {
    const picked = await open({
      directory: true,
      multiple: false,
      title: "选择技能文件夹（包含 SKILL.md）",
    });
    if (typeof picked === "string") {
      await apply(() => addSkill(picked), "已添加技能");
    }
  };

  const addFromRepo = () => {
    setSheetInput("");
    setSheet({
      title: "从仓库添加技能",
      desc: "粘贴 GitHub 仓库链接或 owner/repo，克隆后安装其中的技能；用「技能名@owner/repo」可以只装其中一个。",
      inputPlaceholder: "https://github.com/DietrichGebert/ponytail 或 ponytail@DietrichGebert/ponytail",
      confirmLabel: "克隆并安装",
      altLabel: "选择本地文件夹…",
      onAlt: () => {
        setSheet(null);
        void addFromFolder();
      },
      onConfirm: (input) => apply(() => addSkillRepo(input), "已从仓库安装"),
    });
  };

  const archiveCurrent = (name: string) => {
    setSheet({
      title: "归档技能？",
      desc: `${name} 会移出中央库，指向它的链接会一并移除。归档后可以恢复或删除。`,
      warn: "Agent 目录里的独立副本不会被删除，仍会保留。",
      confirmLabel: "归档",
      onConfirm: async () => {
        if (await apply(() => archiveSkill(name), "已归档")) setDetail(null);
      },
    });
  };

  const restoreArchived = (name: string) => {
    void apply(() => restoreSkill(name), "已恢复到中央库");
  };

  const deleteArchivedSkill = (name: string) => {
    setSheet({
      title: "删除归档中的技能？",
      desc: `${name} 会被移到系统回收站，中央库和归档里都不再保留。`,
      warn: "这是唯一真实副本，只能从回收站找回。",
      confirmLabel: "删除",
      onConfirm: () => apply(() => deleteArchived(name), "已删除"),
    });
  };

  const renderView = () => {
    if (!data) return null;
    if (view === "library") {
      return current ? (
        <Detail
          data={data}
          skill={current}
          onBack={() => setDetail(null)}
          onCreateLink={(agent) =>
            void apply(() => createLink(current.name, agent), "已创建链接")
          }
          onRemoveLink={(agent) => removeAgentLink(current.name, agent)}
          onLinkAll={(agents) => void linkAllAgents(current.name, agents)}
          onUnlinkAll={(agents) => unlinkAllAgents(current.name, agents)}
          onReplace={(agent) => replaceCopy(current.name, agent)}
          onReveal={() => {
            revealSkill(current.name).catch((error) => showToast(String(error)));
          }}
          onRevealAgent={(agent) => {
            revealAgent(agent).catch((error) => showToast(String(error)));
          }}
          onCopyPath={() => void copyPath(current.name)}
          onArchive={() => archiveCurrent(current.name)}
          onOpenSync={() => {
            setView("sync");
            setDetail(null);
          }}
        />
      ) : (
        <Library
          data={data}
          onOpen={setDetail}
          onAdd={addFromRepo}
          onMergeAll={mergeAll}
          onTidy={tidyAll}
          tidying={tidying}
          onSeeOrphans={() => setView("targets")}
          onRefresh={refresh}
          onArchive={archiveCurrent}
          onReveal={(name) => {
            revealSkill(name).catch((error) => showToast(String(error)));
          }}
        />
      );
    }
    if (view === "targets") {
      return (
        <Targets data={data} onOpenAgent={openAgent} onRemoveOrphans={removeOrphanLinks} />
      );
    }
    if (view === "archive") {
      return (
        <Archive data={data} onRestore={restoreArchived} onDelete={deleteArchivedSkill} />
      );
    }
    if (view === "sync")
      return (
        <SyncView data={data} sync={sync} autoSync={autoSync} onConnect={connectSync} onSync={runSync} />
      );
    return (
      <Settings
        data={data}
        sync={sync}
        theme={theme}
        autoSync={autoSync}
        onChangeAutoSync={changeAutoSync}
        onChangeRoot={() => void changeRoot()}
        onTheme={(next) => {
          setTheme(next);
          applyTheme(next);
        }}
        onToast={showToast}
      />
    );
  };

  return (
    <div className="app">
      <aside className="sidebar">
        <div className="brand">
          <span className="appicon">
            <img src={logo} alt="" />
          </span>
          <div>
            <b>Skill Manager</b>
            <span>本机技能</span>
          </div>
        </div>
        <nav className="nav">
          {NAV.map((item) => (
            <button
              key={item.id}
              className={view === item.id ? "on" : undefined}
              onClick={() => {
                setView(item.id);
                setDetail(null);
              }}
            >
              <span className={`nvi ${item.tint}`}>
                <Icon name={item.icon} />
              </span>
              {item.label}
              {item.id === "targets" && orphanCount > 0 && (
                <span className="badge">{orphanCount}</span>
              )}
              {item.id === "sync" && !sync?.remote && (
                <span className="ndot" title="未连接远端" />
              )}
            </button>
          ))}
        </nav>
        <div className="sidefoot">
          <span>中央库</span>
          <code title={data?.centralRoot}>{data ? formatHome(data.centralRoot, data.home) : "—"}</code>
          <em>
            {data ? `${data.skills.length} 个技能 · ${totalFiles.toLocaleString()} 个文件` : "正在扫描…"}
          </em>
          <div className="proto">
            <i />
            {sync?.remote ? "已连接远端仓库" : "v1.1 · 仅本地"}
          </div>
        </div>
      </aside>
      <main className="main">
        {loadError ? (
          <div className="page">
            <div className="errorpage">
              <b>读取中央库失败</b>
              <p>{loadError}</p>
              <button className="btn primary" onClick={() => void refresh()}>
                重试
              </button>
            </div>
          </div>
        ) : data ? (
          renderView()
        ) : (
          <div className="page loading">正在扫描中央库…</div>
        )}
      </main>

      {sheet && (
        <div
          className="overlay show"
          onMouseDown={(event) => {
            if (event.target === event.currentTarget) setSheet(null);
          }}
        >
          <div className="sheet" role="dialog" aria-modal="true" aria-label={sheet.title}>
            <h2>{sheet.title}</h2>
            <p className="sd">{sheet.desc}</p>
            {sheet.inputPlaceholder !== undefined && (
              <input
                className="sheet-input"
                value={sheetInput}
                onChange={(event) => setSheetInput(event.currentTarget.value)}
                placeholder={sheet.inputPlaceholder}
                spellCheck={false}
                autoFocus
              />
            )}
            {sheet.warn && (
              <div className="warnbox">
                <Icon name="warn" />
                <span>{sheet.warn}</span>
              </div>
            )}
            <div className="actions">
              {sheet.altLabel && (
                <button
                  className="btn ghost"
                  onClick={() => {
                    const alt = sheet.onAlt;
                    setSheet(null);
                    alt?.();
                  }}
                >
                  {sheet.altLabel}
                </button>
              )}
              <button className="btn secondary" onClick={() => setSheet(null)}>
                取消
              </button>
              <button
                className="btn primary"
                disabled={sheet.inputPlaceholder !== undefined && !sheetInput.trim()}
                onClick={() => {
                  const action = sheet.onConfirm;
                  const input = sheetInput;
                  setSheet(null);
                  void action(input);
                }}
              >
                {sheet.confirmLabel}
              </button>
            </div>
          </div>
        </div>
      )}
      <div className={toast ? "toast show" : "toast"}>{toast}</div>
    </div>
  );
}
