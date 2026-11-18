const messages = {
  en: { copy: "Copy", copied: "Copied", download: "Download", loading: "Loading…", ended: "This share has ended.", unavailable: "Cannot connect. Keep the sharing computer awake and on the same local network.", manual: "Text selected. Long-press to copy, or press Ctrl/Cmd+C.", remaining: "{time} left", text: "Shared text", image: "Shared image" },
  "zh-CN": { copy: "复制", copied: "已复制", download: "下载", loading: "正在加载…", ended: "分享已结束", unavailable: "暂时无法连接，请确认分享电脑保持唤醒，且设备在同一局域网。", manual: "已选中文本，请长按复制，或按 Ctrl/Cmd+C。", remaining: "剩余 {time}", text: "分享的文本", image: "分享的图片" },
  ja: { copy: "コピー", copied: "コピーしました", download: "ダウンロード", loading: "読み込み中…", ended: "共有は終了しました。", unavailable: "接続できません。共有元のパソコンが起動中で、同じネットワークにあることを確認してください。", manual: "テキストを選択しました。長押しまたは Ctrl/Cmd+C でコピーしてください。", remaining: "残り {time}", text: "共有テキスト", image: "共有画像" },
  ko: { copy: "복사", copied: "복사됨", download: "다운로드", loading: "불러오는 중…", ended: "공유가 종료되었습니다.", unavailable: "연결할 수 없습니다. 공유 컴퓨터가 켜져 있고 같은 네트워크에 연결되어 있는지 확인하세요.", manual: "텍스트가 선택되었습니다. 길게 누르거나 Ctrl/Cmd+C로 복사하세요.", remaining: "{time} 남음", text: "공유 텍스트", image: "공유 이미지" },
  de: { copy: "Kopieren", copied: "Kopiert", download: "Herunterladen", loading: "Wird geladen…", ended: "Diese Freigabe ist beendet.", unavailable: "Keine Verbindung. Der freigebende Computer muss wach und im selben lokalen Netzwerk sein.", manual: "Text ausgewählt. Zum Kopieren lange drücken oder Strg/Cmd+C verwenden.", remaining: "Noch {time}", text: "Geteilter Text", image: "Geteiltes Bild" },
  fr: { copy: "Copier", copied: "Copié", download: "Télécharger", loading: "Chargement…", ended: "Ce partage est terminé.", unavailable: "Connexion impossible. Gardez l’ordinateur de partage éveillé et sur le même réseau local.", manual: "Texte sélectionné. Faites un appui long ou appuyez sur Ctrl/Cmd+C pour copier.", remaining: "Il reste {time}", text: "Texte partagé", image: "Image partagée" },
  es: { copy: "Copiar", copied: "Copiado", download: "Descargar", loading: "Cargando…", ended: "Este enlace ha caducado.", unavailable: "No se puede conectar. Mantén el ordenador encendido y en la misma red local.", manual: "Texto seleccionado. Mantén pulsado o usa Ctrl/Cmd+C para copiar.", remaining: "Quedan {time}", text: "Texto compartido", image: "Imagen compartida" },
};

// HTTP LAN pages may not expose the modern Clipboard API. Keep the legacy attempt
// inside the user's click, then leave the original text selected if both fail.
export async function copySharedText(text, element, environment) {
  const { clipboard, execCommand } = environment;
  if (clipboard?.writeText) {
    try { await clipboard.writeText(text); return true; } catch { /* Try selection-based copy. */ }
  }
  element.focus();
  element.select();
  element.setSelectionRange(0, text.length);
  try { return execCommand("copy") === true; } catch { return false; }
}

export function remainingTime(expiresAt, now = Date.now()) {
  const seconds = Math.max(0, Math.ceil((expiresAt - now) / 1000));
  return `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, "0")}`;
}

async function initialize() {
  const locale = navigator.language.toLowerCase();
  const language = locale.startsWith("zh") ? "zh-CN" : locale.split("-")[0];
  const t = messages[language] || messages.en;
  document.documentElement.lang = messages[language] ? language : "en";
  const element = (id) => document.getElementById(id);
  const message = element("message");
  const copy = element("copy");
  const download = element("download");
  const text = element("text");
  const base = location.pathname.replace(/\/$/, "");
  let expiresAt = 0;
  let timer;
  let ended = false;
  let checking = false;
  let ticks = 0;
  let feedbackTimer;
  message.textContent = t.loading;

  function finish() {
    ended = true;
    clearInterval(timer);
    clearTimeout(feedbackTimer);
    copy.disabled = true;
    download.hidden = true;
    element("actions").hidden = true;
    element("expiry").textContent = "";
    message.textContent = t.ended;
  }

  async function fetchWithTimeout(path) {
    const controller = new AbortController();
    const timeout = setTimeout(() => controller.abort(), 4000);
    try { return await fetch(`${base}/${path}`, { cache: "no-store", signal: controller.signal }); }
    finally { clearTimeout(timeout); }
  }

  async function checkStatus() {
    if (checking || ended) return;
    checking = true;
    try {
      const response = await fetchWithTimeout("status");
      if (!response.ok) finish();
      else if (message.textContent === t.unavailable) message.textContent = "";
    } catch {
      if (!ended) message.textContent = t.unavailable;
    } finally { checking = false; }
  }

  function updateTime() {
    if (Date.now() >= expiresAt) { finish(); return; }
    element("expiry").textContent = t.remaining.replace("{time}", remainingTime(expiresAt));
    if (++ticks % 5 === 0) void checkStatus();
  }

  try {
    const response = await fetchWithTimeout("data");
    if (!response.ok) { finish(); return; }
    const data = await response.json();
    // A phone's clock can differ from the sender's. Use the server's remaining time.
    expiresAt = Date.now() + Math.max(0, data.expiresAt - data.serverTime);
    if (Date.now() >= expiresAt) { finish(); return; }
    const content = data.content;
    message.textContent = "";
    element("content").hidden = false;
    element("actions").hidden = false;
    if (content.name) {
      element("name").textContent = content.name;
      element("name").hidden = false;
    }
    if (content.text !== null) {
      text.value = content.text;
      text.setAttribute("aria-label", t.text);
      text.hidden = false;
      copy.textContent = t.copy;
      copy.hidden = false;
      copy.addEventListener("click", async () => {
        if (Date.now() >= expiresAt || ended) { finish(); return; }
        copy.disabled = true;
        const success = await copySharedText(content.text, text, {
          clipboard: navigator.clipboard,
          execCommand: (command) => document.execCommand(command),
        });
        if (ended) return;
        copy.disabled = false;
        copy.textContent = success ? t.copied : t.copy;
        message.textContent = success ? "" : t.manual;
        clearTimeout(feedbackTimer);
        feedbackTimer = setTimeout(() => { copy.textContent = t.copy; }, 1800);
      });
    } else {
      download.textContent = t.download;
      download.href = `${base}/download`;
      download.hidden = false;
      download.addEventListener("click", (event) => {
        if (Date.now() >= expiresAt || ended) { event.preventDefault(); finish(); }
      });
      const bytes = content.size;
      element("size").textContent = bytes < 1024 ? `${bytes} B` : bytes < 1024 ** 2 ? `${(bytes / 1024).toFixed(1)} KB` : `${(bytes / 1024 ** 2).toFixed(1)} MB`;
      element("size").hidden = false;
      if (content.kind === "image") {
        element("image").src = `${base}/image`;
        element("image").alt = t.image;
        element("image").hidden = false;
      }
    }
    timer = setInterval(updateTime, 1000);
    updateTime();
    document.addEventListener("visibilitychange", () => { if (!document.hidden && !ended) { updateTime(); void checkStatus(); } });
    window.addEventListener("pagehide", () => { clearInterval(timer); clearTimeout(feedbackTimer); });
    window.addEventListener("pageshow", () => {
      if (ended) return;
      clearInterval(timer);
      timer = setInterval(updateTime, 1000);
      updateTime();
      void checkStatus();
    });
  } catch { message.textContent = t.unavailable; }
}

if (typeof document !== "undefined") void initialize();
