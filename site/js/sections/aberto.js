// BUILD IN PUBLIC (downloads) e SUPPORT (doação em Bitcoin).
// Os arquivos vêm de downloads/payload.js, gerado por tools/build_downloads.py.
// Dentro do Claude, o download passa pela confirmação do visualizador (capability "downloads");
// num servidor comum, vira um link de download normal.
import { BITCOIN } from "../data.js";

export function iniciar({ t, aoMudarIdioma }) {
  const aviso = document.getElementById("aviso-baixar");
  let dados = null, meta = null;
  const carregar = async () => { if (!dados) dados = (await import("../../downloads/payload.js")).default; return dados; };

  // mostra tamanho e número de arquivos do pacote sem carregar o pacote inteiro
  import("../../downloads/manifesto.js").then((m) => { meta = m.default; mostrarMeta(); }).catch(() => {});
  function mostrarMeta() {
    if (!meta) return;
    document.getElementById("meta-zip").textContent = `auron-codigo.zip · ${meta.kb} KB · ${t("aberto.arquivos", { n: meta.arquivos })} · commit ${meta.commit}`;
  }

  let downloads = null;
  const claude = globalThis.claude;
  if (claude && typeof claude.use === "function") claude.use("downloads").then((d) => { downloads = d; }).catch(() => {});

  function blob(item) {
    if (item.b64) { const bin = atob(item.b64), b = new Uint8Array(bin.length); for (let i = 0; i < bin.length; i++) b[i] = bin.charCodeAt(i); return new Blob([b], { type: "application/zip" }); }
    return new Blob([item.texto], { type: "text/markdown;charset=utf-8" });
  }
  const diz = (chave, vars, classe = "") => { aviso.className = "aviso-baixar " + classe; aviso.textContent = t(chave, vars); };

  document.querySelectorAll("[data-baixar]").forEach((b) => b.addEventListener("click", async () => {
    diz("aberto.carregando");
    let item;
    try { item = (await carregar())[b.dataset.baixar]; } catch { diz("aberto.indisponivel", null, "erro"); return; }
    if (downloads) {
      diz("aberto.confirmar");
      try { await downloads.save({ filename: item.nome, data: blob(item) }); diz("aberto.salvo", { f: item.nome }, "ok"); }
      catch (e) {
        const c = e && e.code;
        if (c === "declined") diz("aberto.cancelado");
        else if (c === "rate_limited") diz("aberto.ocupado", null, "erro");
        else diz("aberto.indisponivel", null, "erro");
      }
      return;
    }
    const url = URL.createObjectURL(blob(item));
    const a = Object.assign(document.createElement("a"), { href: url, download: item.nome, rel: "noopener" });
    document.body.append(a); a.click(); a.remove();
    setTimeout(() => URL.revokeObjectURL(url), 30000);
    diz("aberto.salvo", { f: item.nome }, "ok");
  }));

  // ---------- doação ----------
  const qr = document.getElementById("qr");
  function desenharQr() {
    if (typeof globalThis.qrcode !== "function" || qr.childElementCount) return;
    try {
      const q = globalThis.qrcode(0, "M"); q.addData("bitcoin:" + BITCOIN); q.make();
      qr.innerHTML = q.createSvgTag({ cellSize: 4, margin: 2, scalable: true });
    } catch { qr.replaceChildren(); }
  }
  if (document.readyState === "complete") desenharQr(); else window.addEventListener("load", desenharQr);
  document.getElementById("copiar").addEventListener("click", async () => {
    const av = document.getElementById("aviso-copiar");
    try { await navigator.clipboard.writeText(BITCOIN); av.className = "aviso-baixar ok"; av.textContent = t("apoio.copiado"); }
    catch {
      const sel = getSelection(), faixa = document.createRange();
      faixa.selectNodeContents(document.getElementById("endereco")); sel.removeAllRanges(); sel.addRange(faixa);
      av.className = "aviso-baixar"; av.textContent = t("apoio.selecionado");
    }
  });
  aoMudarIdioma(() => { mostrarMeta(); });
}
