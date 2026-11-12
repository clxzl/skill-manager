import { useEffect, useState, type CSSProperties, type FormEvent } from "react";
import {
  autostartEnabled,
  formatDate,
  formatHome,
  formatSize,
  setAutostart,
  skillColor,
  skillGlyph,
  type AgentInfo,
  type ScanResult,
  type SkillInfo,
  type Status,
  type SyncStatus,
} from "./api";
import { Icon, type IconName } from "./icons";
import type { Theme } from "./theme";
import claudeIcon from "./assets/agents/claude.svg";
import cursorIcon from "./assets/agents/cursor.svg";
import deepseekIcon from "./assets/agents/deepseek.svg";
import openaiIcon from "./assets/agents/openai.svg";
import qoderIcon from "./assets/agents/qoder.svg";
import traeIcon from "./assets/agents/trae.svg";
import workbuddyIcon from "./assets/agents/workbuddy.svg";
import zcodeIcon from "./assets/agents/zcode.png";

const agentIcons: Record<string, string> = {
  claude: claudeIcon,
  codex: openaiIcon,
  qoder: qoderIcon,
  "qoder-cn": qoderIcon,
  cursor: cursorIcon,
  zcode: zcodeIcon,
  deepseek: deepseekIcon,
  workbuddy: workbuddyIcon,
  "workbuddy-ai": workbuddyIcon,
  trae: traeIcon,
  "trae-cn": traeIcon,
};

function AgentAvatar({ agent, className }: { agent: AgentInfo; className: string }) {
  const src = agentIcons[agent.id];
  return (
    <span className={className} title={agent.name}>
      {src ? <img src={src} alt="" /> : agent.abbr}
    </span>
  );
}

const statusMeta: Record<Status, { label: string; icon: IconName }> = {
  link: { label: "已链接", icon: "link" },
  copy: { label: "独立副本", icon: "copy" },
  none: { label: "未安装", icon: "dash" },
};

const THEMES: { value: Theme; label: string }[] = [
  { value: "system", label: "跟随系统" },
  { value: "light", label: "浅色" },
  { value: "dark", label: "深色" },
];

function SkillIcon({ skill, index, big }: { skill: SkillInfo; index: number; big?: boolean }) {
  const style = { "--c": skillColor(index) } as CSSProperties;
  return (
    <span className={big ? "dicon" : "sicon"} style={style}>
      {skillGlyph(skill.name)}
    </span>
  );
}

function countByStatus(data: ScanResult) {
  let links = 0;
  let copies = 0;
  for (const skill of data.skills) {
    for (const agent of data.agents) {
      const status = skill.agents[agent.id] ?? "none";
      if (status === "link") links += 1;
      if (status === "copy") copies += 1;
    }
  }
  return { links, copies, orphans: data.orphans.length };
}

const compactCount = (value: number) =>
  value >= 1000 ? `${(value / 1000).toFixed(1)}k` : String(value);

const totalFiles = (data: ScanResult) =>
  data.skills.reduce((sum, skill) => sum + skill.files, 0);
const totalSize = (data: ScanResult) =>
  data.skills.reduce((sum, skill) => sum + skill.sizeBytes, 0);

export function Library({
  data,
  onOpen,
  onAdd,
  onMergeAll,
  onSeeOrphans,
  onRefresh,
  onArchive,
  onReveal,
}: {
  data: ScanResult;
  onOpen: (name: string) => void;
  onAdd: () => void;
  onMergeAll: (skills: string[]) => void;
  onSeeOrphans: () => void;
  onRefresh: () => Promise<void>;
  onArchive: (name: string) => void;
  onReveal: (name: string) => void;
}) {
  const [query, setQuery] = useState("");
  const [refreshing, setRefreshing] = useState(false);
  const totals = countByStatus(data);
  const copiesBySkill = data.skills
    .filter((skill) => Object.values(skill.agents).includes("copy"))
    .map((skill) => skill.name);
  const filtered = data.skills.filter((skill) =>
    skill.name.toLowerCase().includes(query.trim().toLowerCase()),
  );

  return (
    <div className="page">
      <div className="page-head">
        <div>
          <h1>技能库</h1>
          <p className="sub">中央库是唯一真实副本，链接分发到 {data.agents.length} 个 Agent。</p>
        </div>
        <div className="head-actions">
          <button className="btn primary" onClick={onAdd}>
            <Icon name="plus" />
            添加技能
          </button>
          <label className="search">
            <Icon name="search" />
            <input
              value={query}
              onChange={(event) => setQuery(event.currentTarget.value)}
              placeholder="搜索技能"
              aria-label="搜索技能"
            />
          </label>
        </div>
      </div>
      <div className="statsline">
        <span>
          <i className="dot blue" />
          <b>{data.skills.length}</b> 个技能
        </span>
        <span>
          <i className="dot blue" />
          <b>{totals.links}</b> 个链接
        </span>
        <span>
          <i className="dot orange" />
          <b>{totals.copies}</b> 个独立副本
        </span>
        <span>
          <i className="dot orange" />
          <b>{totals.orphans}</b> 个孤立链接
        </span>
      </div>
      {(totals.copies > 0 || totals.orphans > 0) && (
        <div className="alert">
          <span className="alert-ic">
            <Icon name="warn" />
          </span>
          <div className="alert-copy">
            <b>
              {totals.copies > 0
                ? `有 ${totals.copies} 个独立副本`
                : `有 ${totals.orphans} 个孤立链接`}
            </b>
            <p>
              {totals.copies > 0
                ? `${copiesBySkill.join("、")} 在 Agent 目录中是副本，中央库的修改不会同步过去。`
                : ""}
              {totals.orphans > 0
                ? `${totals.copies > 0 ? " " : ""}Agent 目录里还有 ${totals.orphans} 个链接指向中央库之外。`
                : ""}
            </p>
          </div>
          {totals.copies > 0 && (
            <button className="btn primary" onClick={() => onMergeAll(copiesBySkill)}>
              合并为链接
            </button>
          )}
          {totals.orphans > 0 && (
            <button className="btn secondary" onClick={onSeeOrphans}>
              查看孤立链接
            </button>
          )}
        </div>
      )}
      <section className="group">
        <div className="group-head">
          <span className="gh-title">
            <b>全部技能</b>
            <button
              className="btn ghost"
              disabled={refreshing}
              onClick={() => {
                setRefreshing(true);
                void onRefresh().finally(() => setRefreshing(false));
              }}
            >
              <Icon name="sync" className={refreshing ? "spin" : undefined} />
              刷新
            </button>
          </span>
          <span>单击卡片查看安装位置</span>
        </div>
        {filtered.length > 0 && (
          <div className="cards">
            {filtered.map((skill) => {
              const linked = data.agents.filter((agent) => skill.agents[agent.id] === "link");
              const copies = data.agents.filter(
                (agent) => skill.agents[agent.id] === "copy",
              ).length;
              return (
                <div
                  className="card"
                  key={skill.name}
                  role="button"
                  tabIndex={0}
                  onClick={() => onOpen(skill.name)}
                  onKeyDown={(event) => {
                    if (event.target !== event.currentTarget) return;
                    if (event.key === "Enter" || event.key === " ") {
                      event.preventDefault();
                      onOpen(skill.name);
                    }
                  }}
                >
                  <div className="card-head">
                    <SkillIcon skill={skill} index={data.skills.indexOf(skill)} />
                    <span className="nm">
                      <b>{skill.name}</b>
                      <em>
                        {formatSize(skill.sizeBytes)} · {skill.files.toLocaleString()} 个文件 ·{" "}
                        {formatDate(skill.modified)}修改
                      </em>
                    </span>
                    <span className="card-acts">
                      <button
                        title="归档"
                        aria-label={`归档 ${skill.name}`}
                        onClick={(event) => {
                          event.stopPropagation();
                          onArchive(skill.name);
                        }}
                      >
                        <Icon name="archive" />
                      </button>
                      <button
                        title="在资源管理器中显示"
                        aria-label={`在资源管理器中显示 ${skill.name}`}
                        onClick={(event) => {
                          event.stopPropagation();
                          onReveal(skill.name);
                        }}
                      >
                        <Icon name="open" />
                      </button>
                    </span>
                    <span className="chev">
                      <Icon name="chev" />
                    </span>
                  </div>
                  {skill.description && <p className="card-desc">{skill.description}</p>}
                  <div className="card-foot">
                    <span className="left">
                      {linked.length > 0 && (
                        <span className="stack">
                          {linked.slice(0, 6).map((agent) => (
                            <AgentAvatar key={agent.id} agent={agent} className="stack-icon" />
                          ))}
                          {linked.length > 6 && (
                            <span className="stack-icon stack-more">+{linked.length - 6}</span>
                          )}
                        </span>
                      )}
                      {copies > 0 && <span className="copy-pill">{copies} 个副本</span>}
                      {linked.length === 0 && copies === 0 && <span className="none">未安装</span>}
                    </span>
                  </div>
                </div>
              );
            })}
          </div>
        )}
        {filtered.length === 0 && (
          <div className="empty">
            <Icon name="stack" />
            <b>{data.skills.length === 0 ? "中央库还没有技能" : "没有匹配的技能"}</b>
            <p>
              {data.skills.length === 0
                ? "点右上角「添加技能」，选择包含 SKILL.md 的文件夹。"
                : "换个关键词试试。"}
            </p>
          </div>
        )}
      </section>
      <div style={{ height: 26 }} />
    </div>
  );
}

export function Detail({
  data,
  skill,
  onBack,
  onCreateLink,
  onReplace,
  onReveal,
  onRevealAgent,
  onCopyPath,
  onOpenSync,
  onArchive,
}: {
  data: ScanResult;
  skill: SkillInfo;
  onBack: () => void;
  onCreateLink: (agent: string) => void;
  onReplace: (agent: string) => void;
  onReveal: () => void;
  onRevealAgent: (agent: string) => void;
  onCopyPath: () => void;
  onOpenSync: () => void;
  onArchive: () => void;
}) {
  const index = data.skills.indexOf(skill);
  const absolute = `${data.centralRoot}\\${skill.name}`;

  return (
    <div className="page">
      <button className="back" onClick={onBack}>
        <Icon name="back" />
        技能库
      </button>
      <div className="dhead">
        <SkillIcon skill={skill} index={index} big />
        <div className="t">
          <h1>{skill.name}</h1>
          {skill.description && <p className="ddesc">{skill.description}</p>}
          <div className="meta">
            {formatSize(skill.sizeBytes)} · {skill.files.toLocaleString()} 个文件 ·{" "}
            {formatDate(skill.modified)}修改
          </div>
        </div>
        <div className="acts">
          <button className="btn secondary" onClick={onArchive}>
            <Icon name="archive" />
            归档
          </button>
          <button className="btn secondary" onClick={onReveal}>
            <Icon name="open" />
            在资源管理器中显示
          </button>
        </div>
      </div>
      <div className="pathrow">
        <code>{absolute}</code>
        <button className="btn ghost" onClick={onCopyPath}>
          复制
        </button>
      </div>
      <div className="cols">
        <div className="colmain">
          <section className="group">
            <div className="group-head">
              <b>安装位置</b>
              <span>{data.agents.length} 个 Agent</span>
            </div>
            {data.agents.map((agent) => {
              const status = skill.agents[agent.id] ?? "none";
              const meta = statusMeta[status];
              return (
                <div className="arow" key={agent.id}>
                  <AgentAvatar agent={agent} className="avatar" />
                  <span className="a-t">
                    <b>{agent.name}</b>
                    <code>
                      {formatHome(agent.dir, data.home)}/{skill.name}
                    </code>
                  </span>
                  <span className={`pill ${status}`}>
                    <Icon name={meta.icon} />
                    {meta.label}
                  </span>
                  {status === "link" && (
                    <button className="btn ghost" onClick={() => onRevealAgent(agent.id)}>
                      打开目录
                    </button>
                  )}
                  {status === "copy" && (
                    <button className="btn ghost" onClick={() => onReplace(agent.id)}>
                      替换为链接
                    </button>
                  )}
                  {status === "none" && (
                    <button className="btn ghost" onClick={() => onCreateLink(agent.id)}>
                      创建链接
                    </button>
                  )}
                </div>
              );
            })}
          </section>
          <section className="group">
            <div className="group-head">
              <b>详细信息</b>
            </div>
            <div className="irow">
              <span className="k">大小</span>
              <span className="v">{formatSize(skill.sizeBytes)}</span>
            </div>
            <div className="irow">
              <span className="k">文件</span>
              <span className="v">{skill.files.toLocaleString()} 个</span>
            </div>
            <div className="irow">
              <span className="k">修改时间</span>
              <span className="v">{formatDate(skill.modified)}</span>
            </div>
            <div className="irow">
              <span className="k">中央库</span>
              <span className="v">
                <code>{formatHome(absolute, data.home)}</code>
              </span>
            </div>
          </section>
        </div>
        <div className="colside">
          <section className="group">
            <div className="group-head">
              <b>文件</b>
              <span>{skill.files.toLocaleString()} 个</span>
            </div>
            <div className="arow" style={{ minHeight: 56 }}>
              <span className="avatar filebadge">MD</span>
              <span className="a-t">
                <b>SKILL.md</b>
                <code>{formatHome(`${absolute}\\SKILL.md`, data.home)}</code>
              </span>
            </div>
            {skill.files > 1 && (
              <div className="arow" style={{ minHeight: 56 }}>
                <span className="avatar filebadge">+{compactCount(skill.files - 1)}</span>
                <span className="a-t">
                  <b>其余 {(skill.files - 1).toLocaleString()} 个文件</b>
                  <code>随技能一起存放</code>
                </span>
              </div>
            )}
            <p className="rule">
              修改中央库会立即作用于所有链接；移除技能只删除链接，不删除中央库。
            </p>
          </section>
          <section className="group">
            <div className="group-head">
              <b>同步</b>
            </div>
            <div className="syncbox">
              <b>{data.agents.length} 个 Agent 共用中央库</b>
              <p>在「同步」页连接私有 Git 仓库，即可把中央库推送到远端。</p>
              <button className="btn secondary" onClick={onOpenSync}>
                查看同步状态
              </button>
            </div>
          </section>
        </div>
      </div>
      <div style={{ height: 26 }} />
    </div>
  );
}

export function Targets({
  data,
  onOpenAgent,
  onRemoveOrphans,
}: {
  data: ScanResult;
  onOpenAgent: (agent: string) => void;
  onRemoveOrphans: (agent: string, count: number) => void;
}) {
  const totals = countByStatus(data);

  return (
    <div className="page">
      <div className="page-head">
        <div>
          <h1>目标 Agent</h1>
          <p className="sub">
            {data.agents.length} 个 Agent 目录，共 {totals.links} 个链接、{totals.copies} 个独立副本。
          </p>
        </div>
      </div>
      <div className="statsline">
        <span>
          <i className="dot blue" />
          <b>{totals.links}</b> 个链接
        </span>
        <span>
          <i className="dot orange" />
          <b>{totals.copies}</b> 个独立副本
        </span>
        <span>
          <i className="dot orange" />
          <b>{totals.orphans}</b> 个孤立链接
        </span>
      </div>
      <section className="group" style={{ marginTop: 2 }}>
        {data.agents.map((agent) => {
          const installed = data.skills.filter(
            (skill) => (skill.agents[agent.id] ?? "none") !== "none",
          );
          const links = installed.filter((skill) => skill.agents[agent.id] === "link").length;
          const copies = installed.filter((skill) => skill.agents[agent.id] === "copy").length;
          const orphans = data.orphans.filter((orphan) => orphan.agent === agent.id);
          const orphanDir = orphans[0]
            ? orphans[0].target.replace(/[\\/][^\\/]+$/, "") || orphans[0].target
            : "";
          return (
            <div key={agent.id}>
              <div className="target-row">
                <AgentAvatar agent={agent} className="t-av" />
                <span className="t-t">
                  <b>{agent.name}</b>
                  <code>{formatHome(agent.dir, data.home)}</code>
                </span>
                <span className="counts">
                  <b>{installed.length}</b> 个技能 · {links} 个链接
                  {copies > 0 && (
                    <>
                      {" · "}
                      <em>{copies} 个副本</em>
                    </>
                  )}
                  {orphans.length > 0 && (
                    <>
                      {" · "}
                      <em>{orphans.length} 个孤立链接</em>
                    </>
                  )}
                </span>
                <button className="btn secondary" onClick={() => onOpenAgent(agent.id)}>
                  打开目录
                </button>
              </div>
              {orphans.length > 0 && (
                <div className="orphans">
                  <div className="oh">
                    <Icon name="warn" />
                    {orphans.length} 个链接指向中央库之外
                  </div>
                  <div className="op">
                    <code>{formatHome(orphanDir, data.home)}</code>
                    {orphans.length > 1 ? " 等位置" : ""} 不在中央库中。链接不会被自动删除。
                  </div>
                  <div className="opills">
                    {orphans.map((orphan) => (
                      <span className="opill" key={orphan.name}>
                        {orphan.name}
                        <code>→ 旧路径</code>
                      </span>
                    ))}
                  </div>
                  <div className="orphan-actions">
                    <button onClick={() => onRemoveOrphans(agent.id, orphans.length)}>
                      移除 {orphans.length} 个链接
                    </button>
                  </div>
                </div>
              )}
            </div>
          );
        })}
      </section>
      <p className="sub" style={{ marginTop: 14, fontSize: 12 }}>
        孤立链接只存在于 Agent 目录，中央库没有对应技能。移除只删链接，不动中央库。
      </p>
      <div style={{ height: 26 }} />
    </div>
  );
}

export function Archive({
  data,
  onRestore,
  onDelete,
}: {
  data: ScanResult;
  onRestore: (name: string) => void;
  onDelete: (name: string) => void;
}) {
  return (
    <div className="page">
      <div className="page-head">
        <div>
          <h1>归档</h1>
          <p className="sub">
            归档的技能已移出中央库，指向它的链接也一并移除。恢复后可重新创建链接。
          </p>
        </div>
      </div>
      <section className="group" style={{ marginTop: 2 }}>
        <div className="group-head">
          <b>已归档技能</b>
          <span>{data.archived.length} 个</span>
        </div>
        {data.archived.map((skill, index) => (
          <div className="arow" key={skill.name}>
            <SkillIcon skill={skill} index={index} />
            <span className="a-t">
              <b>{skill.name}</b>
              <code>
                {formatSize(skill.sizeBytes)} · {skill.files.toLocaleString()} 个文件 ·{" "}
                {formatDate(skill.modified)}修改
              </code>
            </span>
            <button className="btn ghost" onClick={() => onRestore(skill.name)}>
              恢复
            </button>
            <button className="btn ghost" onClick={() => onDelete(skill.name)}>
              删除
            </button>
          </div>
        ))}
        {data.archived.length === 0 && (
          <div className="empty">
            <Icon name="archive" />
            <b>归档是空的</b>
            <p>在技能详情页点「归档」，技能会移到这里。</p>
          </div>
        )}
      </section>
      <p className="sub" style={{ fontSize: 12, marginTop: 14 }}>
        归档位置：<code>{formatHome(data.archiveRoot, data.home)}</code>。删除会移到系统回收站。
      </p>
      <div style={{ height: 26 }} />
    </div>
  );
}

export function SyncView({
  data,
  sync,
  onConnect,
  onSync,
}: {
  data: ScanResult;
  sync: SyncStatus | null;
  onConnect: (remote: string, username: string, token: string) => Promise<boolean>;
  onSync: () => Promise<boolean>;
}) {
  const [editing, setEditing] = useState(false);
  const [url, setUrl] = useState("");
  const [user, setUser] = useState("");
  const [token, setToken] = useState("");
  const [busy, setBusy] = useState(false);

  const remote = sync?.remote ?? null;
  const connected = remote !== null;
  const isHttps = remote?.startsWith("https://") ?? false;
  const showForm = !connected || editing;
  const largest = data.skills.reduce<SkillInfo | null>(
    (best, skill) => (!best || skill.sizeBytes > best.sizeBytes ? skill : best),
    null,
  );
  const commits = sync?.commits ?? [];

  const startEdit = () => {
    setUrl(remote ?? "");
    setUser(sync?.remoteUser ?? "");
    setToken("");
    setEditing(true);
  };

  const cancelEdit = () => {
    setEditing(false);
    setToken("");
  };

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    if (!url.trim() || busy) return;
    setBusy(true);
    const ok = await onConnect(url.trim(), user.trim(), token.trim());
    setBusy(false);
    if (ok) {
      setEditing(false);
      setToken("");
    }
  };

  const runNow = async () => {
    if (busy) return;
    setBusy(true);
    await onSync();
    setBusy(false);
  };

  return (
    <div className="page">
      <div className="page-head">
        <div>
          <h1>同步</h1>
          <p className="sub">中央库与私有 Git 仓库双向同步，冲突时由你决定。</p>
        </div>
      </div>

      {showForm ? (
        <form className="sync-hero form" onSubmit={(event) => void submit(event)}>
          <span className="cloud">
            <Icon name="cloud" />
          </span>
          <div className="sync-form">
            <div className="h wide">
              <h2>{connected ? "更换远端仓库" : "连接远端仓库"}</h2>
              <p>
                中央库本身就是 Git 工作区。地址支持 HTTPS、SSH（git@host:path）和 file://。
                令牌只保存在 Windows 凭据管理器，不会写进仓库或配置。
              </p>
            </div>
            <label className="wide">
              仓库地址
              <input
                value={url}
                onChange={(event) => setUrl(event.currentTarget.value)}
                placeholder="https://github.com/you/skills.git"
                spellCheck={false}
                autoFocus
              />
            </label>
            <label>
              用户名
              <input
                value={user}
                onChange={(event) => setUser(event.currentTarget.value)}
                placeholder="Gitee 填用户名；GitHub 可留空"
                spellCheck={false}
              />
            </label>
            <label>
              访问令牌
              <input
                type="password"
                value={token}
                onChange={(event) => setToken(event.currentTarget.value)}
                placeholder={
                  connected && sync?.credential
                    ? "留空则沿用已保存的凭据"
                    : "PAT，仅保存在系统凭据管理器"
                }
                autoComplete="off"
              />
            </label>
            {sync && !sync.gitAvailable && (
              <p className="form-warn wide">
                未检测到 Git，请先安装 Git for Windows 并重启应用。
              </p>
            )}
            <div className="form-actions wide">
              <button
                className="btn primary"
                type="submit"
                disabled={busy || !url.trim() || sync?.gitAvailable === false}
              >
                {busy ? "连接中…" : connected ? "保存并拉取" : "连接并拉取"}
              </button>
              {editing && (
                <button className="btn secondary" type="button" onClick={cancelEdit}>
                  取消
                </button>
              )}
            </div>
          </div>
        </form>
      ) : (
        <div className="sync-hero">
          <span className="cloud">
            <Icon name="cloud" />
          </span>
          <div className="h">
            <h2>已连接远端仓库</h2>
            <p>
              <code>{remote}</code> · 分支 {sync?.branch}
            </p>
            <p className="credline">
              {isHttps
                ? sync?.credential
                  ? "凭据已保存到 Windows 凭据管理器"
                  : "未保存访问令牌，推送可能失败；点「更换仓库」重新填写"
                : "使用 SSH 密钥认证，由系统 ssh-agent 管理"}
            </p>
          </div>
          <div className="hero-actions">
            <button className="btn primary" onClick={() => void runNow()} disabled={busy}>
              {busy ? "同步中…" : "立即同步"}
            </button>
            <button className="btn secondary" onClick={startEdit} disabled={busy}>
              更换仓库
            </button>
          </div>
        </div>
      )}

      <div className="grid2">
        <section className="group">
          <div className="group-head">
            <b>{connected ? "本地状态" : "首次同步"}</b>
            <span>{connected ? `分支 ${sync?.branch}` : "等待连接"}</span>
          </div>
          {connected ? (
            <>
              <div className="stats3">
                <div>
                  <b>{sync?.dirty ?? 0}</b>
                  <span>未提交改动</span>
                </div>
                <div>
                  <b>{sync?.ahead ?? 0}</b>
                  <span>待推送</span>
                </div>
                <div>
                  <b>{sync?.behind ?? 0}</b>
                  <span>待拉取</span>
                </div>
              </div>
              <p className="hint">
                同步顺序：提交本地改动 → 拉取远端 → 变基 → 推送。冲突时停止并保留远端提交，不会强推。
              </p>
            </>
          ) : (
            <>
              <div className="stats3">
                <div>
                  <b>{data.skills.length}</b>
                  <span>个技能</span>
                </div>
                <div>
                  <b>{totalFiles(data).toLocaleString()}</b>
                  <span>个文件</span>
                </div>
                <div>
                  <b>约 {formatSize(totalSize(data))}</b>
                  <span>总大小</span>
                </div>
              </div>
              <p className="hint">
                连接后首次推送会创建 1 个提交，信息为{" "}
                <code>sync: 首次推送 {data.skills.length} 个技能</code>。
                {largest ? ` ${largest.name} 占 ${formatSize(largest.sizeBytes)}。` : ""}
              </p>
            </>
          )}
        </section>
        <section className="group">
          <div className="group-head">
            <b>最近提交</b>
            <span>{commits.length} 条</span>
          </div>
          {commits.map((commit) => (
            <div className="sync-commit" key={commit.hash}>
              <code>{commit.hash}</code>
              <span>{commit.message}</span>
              <em>{formatDate(commit.time)}</em>
            </div>
          ))}
          {commits.length === 0 && (
            <div className="empty">
              <Icon name="clock" />
              <b>{connected ? "还没有提交" : "还没有同步记录"}</b>
              <p>
                {connected
                  ? "点「立即同步」创建第一个提交。"
                  : "连接仓库并完成首次推送后，这里会显示提交。"}
              </p>
            </div>
          )}
        </section>
      </div>
      <div style={{ height: 26 }} />
    </div>
  );
}

export function Settings({
  data,
  sync,
  theme,
  onChangeRoot,
  onTheme,
  onToast,
}: {
  data: ScanResult;
  sync: SyncStatus | null;
  theme: Theme;
  onChangeRoot: () => void;
  onTheme: (theme: Theme) => void;
  onToast: (message: string) => void;
}) {
  const remote = sync?.remote ?? null;
  const isHttps = remote?.startsWith("https://") ?? false;
  const [auto, setAuto] = useState<boolean | null>(null);

  useEffect(() => {
    autostartEnabled()
      .then(setAuto)
      .catch(() => setAuto(false));
  }, []);

  const toggleAuto = async () => {
    const next = !auto;
    setAuto(next);
    try {
      await setAutostart(next);
    } catch (error) {
      setAuto(!next);
      onToast(String(error));
    }
  };

  return (
    <div className="page">
      <div className="page-head">
        <div>
          <h1>设置</h1>
          <p className="sub">外观、启动、中央库、同步与凭据。</p>
        </div>
      </div>
      <div style={{ display: "grid", gap: 18 }}>
        <section className="group">
          <div className="group-head">
            <b>通用</b>
          </div>
          <div className="setrow">
            <div className="s-t">
              <b>主题</b>
              <p>跟随系统，或固定使用浅色、深色。</p>
            </div>
            <div className="seg" role="group" aria-label="主题">
              {THEMES.map((item) => (
                <button
                  key={item.value}
                  className={theme === item.value ? "on" : undefined}
                  aria-pressed={theme === item.value}
                  onClick={() => onTheme(item.value)}
                >
                  {item.label}
                </button>
              ))}
            </div>
          </div>
          <div className="setrow">
            <div className="s-t">
              <b>开机自启</b>
              <p>登录 Windows 后在后台启动，从托盘打开主窗口。</p>
            </div>
            <button
              className={auto ? "switch on" : "switch"}
              role="switch"
              aria-checked={auto === true}
              aria-label="开机自启"
              disabled={auto === null}
              onClick={() => void toggleAuto()}
            >
              <i />
            </button>
          </div>
          <div className="setrow">
            <div className="s-t">
              <b>关闭窗口</b>
              <p>关闭时隐藏到系统托盘；退出请使用托盘菜单。</p>
            </div>
            <span className="val">隐藏到托盘</span>
          </div>
        </section>
        <section className="group">
          <div className="group-head">
            <b>中央库</b>
          </div>
          <div className="setrow">
            <div className="s-t">
              <b>位置</b>
              <p>所有技能的唯一真实副本，也是 Git 工作区。</p>
            </div>
            <span className="val">
              <code>{formatHome(data.centralRoot, data.home)}</code>
            </span>
            <button className="btn secondary" onClick={onChangeRoot}>
              更改
            </button>
          </div>
          <div className="setrow">
            <div className="s-t">
              <b>链接方式</b>
              <p>Windows 上优先使用 Junction，其他系统使用符号链接。</p>
            </div>
            <span className="val">自动（Junction）</span>
          </div>
        </section>
        <section className="group">
          <div className="group-head">
            <b>同步</b>
          </div>
          <div className="setrow">
            <div className="s-t">
              <b>状态</b>
              <p>
                {sync && !sync.gitAvailable
                  ? "未检测到 Git。"
                  : remote
                    ? "已连接远端仓库。"
                    : "未连接远端仓库。"}
              </p>
            </div>
            <span className="val">
              {remote ? <code>{remote}</code> : sync && !sync.gitAvailable ? "缺少 Git" : "未连接"}
            </span>
          </div>
          <div className="setrow">
            <div className="s-t">
              <b>提交信息</b>
              <p>每次同步使用同一种格式。</p>
            </div>
            <span className="val">
              <code>sync: &lt;描述&gt;</code>
            </span>
          </div>
        </section>
        <section className="group">
          <div className="group-head">
            <b>凭据</b>
          </div>
          <div className="setrow">
            <div className="s-t">
              <b>存储位置</b>
              <p>访问令牌只保存在 Windows 凭据管理器，不落盘、不进日志。</p>
            </div>
            <span className="val">
              {remote && isHttps
                ? sync?.credential
                  ? "已保存（凭据管理器）"
                  : "未保存"
                : remote
                  ? "SSH Key（由 Git 管理）"
                  : "尚未配置"}
            </span>
          </div>
        </section>
      </div>
      <p className="sub" style={{ fontSize: 12, marginTop: 16 }}>
        同步不会强推；拉取与推送冲突时会停止，交给你在 Git 中解决。
      </p>
      <div style={{ height: 26 }} />
    </div>
  );
}
