//! O estado do worker em JSON, para a API.

#[allow(clippy::wildcard_imports)]
use super::*;

impl Ultrax {
    // -----------------------------------------------------------------------
    // Estado para o painel
    // -----------------------------------------------------------------------

    /// O estado inteiro em JSON, para `/api/estado`.
    #[allow(clippy::too_many_lines)]
    pub fn json(&self) -> String {
        let mut j = String::with_capacity(8 * 1024);
        let p = self.placar();
        let fila = self.fila.lock().map_or(0, |f| f.len());
        let _ = write!(
            j,
            "{{\"modo\":\"{MODO}\",\"selo\":\"{SELO}\",\"ligado\":{},\"linhas\":{},\"nucleos\":{},\"uso_cpu\":{},\
             \"memoria_mib\":{},\"reservada_mib\":{:.1},\"gpu\":{},\"debug\":{},\"worker\":\"{}\",\"fila\":{fila},\
             \"desafio_a_cada\":{DESAFIO_A_CADA},\"verificador\":\"esta máquina (autoconferência LAB)\",",
            self.ligado.load(Ordering::Relaxed),
            self.linhas.load(Ordering::Relaxed),
            self.nucleos,
            self.uso_cpu.load(Ordering::Relaxed),
            self.memoria_mib.load(Ordering::Relaxed),
            mib(self.reservada.load(Ordering::Relaxed)),
            self.json_gpu(),
            self.debug.load(Ordering::Relaxed),
            hex(&self.worker),
        );
        j.push_str("\"ativas\":[");
        if let Ok(a) = self.ativas.lock() {
            for (k, x) in a.iter().enumerate() {
                if k > 0 {
                    j.push(',');
                }
                let feitas = x.feitas.load(Ordering::Relaxed).min(x.total);
                let _ = write!(
                    j,
                    "{{\"linha\":{},\"dispositivo\":{},\"numero\":{},\"id\":\"{}\",\"categoria\":\"{}\",\"tipo\":\"{}\",\"descricao\":{},\
                     \"tamanho\":{},\"passos\":{},\"resumo\":{},\"metodo\":{},\"desafio\":{},\"estado\":\"{}\",\"feitas\":{feitas},\"total\":{},\
                     \"operacoes\":{},\"inicio\":{},\"memoria_mib\":{:.1},\"entrada\":\"{}\",\"job\":{},\"indice\":{}}}",
                    x.linha,
                    texto_json(x.gpu.as_deref().map_or("CPU", |_| "GPU")),
                    x.numero,
                    hex(&x.id),
                    x.especificacao.tipo().categoria().nome(),
                    x.especificacao.tipo().nome(),
                    texto_json(x.especificacao.tipo().descricao()),
                    x.especificacao.tamanho(),
                    x.especificacao.passos(),
                    texto_json(&x.especificacao.resumo()),
                    texto_json(x.metodo.nome()),
                    x.desafio,
                    x.estado.nome(),
                    x.total,
                    x.operacoes,
                    x.inicio_ms,
                    mib(x.memoria),
                    x.entrada.map(|e| hex(&e)).unwrap_or_default(),
                    x.job.map_or("null".to_string(), |(j, _)| format!("\"{}\"", hex(&j))),
                    x.job.map_or("null".to_string(), |(_, i)| i.to_string()),
                );
            }
        }
        let r = &p.reputacao;
        let _ = write!(
            j,
            "],\"placar\":{{\"liquidadas\":{},\"recusadas\":{},\"canceladas\":{},\"expiradas\":{},\
             \"desafios_certos\":{},\"desafios_errados\":{},\"work_score\":\"{}\",\"operacoes_verificadas\":{},\
             \"operacoes_sem_credito\":{},\"ms_calculo\":{},\"ms_verificacao\":{},\"segundos_ligado\":{},\
             \"matriz\":{},\"mochila\":{},\"difusao\":{},\"ia\":{},\"enviadas\":{},\"verificadas\":{},\"divergentes\":{},\
             \"disputadas\":{},\"nota\":{},\"taxa\":{},\"geradas\":{},\"abandonadas\":{}}},",
            p.liquidadas,
            p.recusadas,
            p.canceladas,
            p.expiradas,
            p.desafios_certos,
            p.desafios_errados,
            p.score.texto(),
            p.score.operacoes_verificadas,
            p.score.operacoes_sem_credito,
            p.ms_calculo,
            p.ms_verificacao,
            p.segundos_ligado,
            p.por_tipo[0],
            p.por_tipo[1],
            p.por_tipo[2],
            p.por_tipo[3],
            r.enviadas,
            r.verificadas,
            r.divergentes,
            r.disputadas,
            r.nota(),
            r.taxa_de_acerto().map_or("null".to_string(), |t| t.to_string()),
            p.geradas,
            p.abandonadas,
        );
        let _ = write!(j, "\"ia\":{},", self.json_ia());
        j.push_str("\"historico\":[");
        if let Ok(h) = self.historico.lock() {
            for (k, x) in h.iter().enumerate() {
                if k > 0 {
                    j.push(',');
                }
                let _ = write!(
                    j,
                    "{{\"numero\":{},\"tipo\":\"{}\",\"categoria\":\"{}\",\"resumo\":{},\"metodo\":{},\"desafio\":{},\
                     \"estado\":\"{}\",\"operacoes\":{},\"ms_calculo\":{},\"fim\":{},\"resultado\":\"{}\",\"nota\":{}}}",
                    x.numero,
                    x.tipo.nome(),
                    x.tipo.categoria().nome(),
                    texto_json(&x.resumo),
                    texto_json(x.metodo.nome()),
                    x.desafio,
                    x.estado.nome(),
                    x.operacoes,
                    x.ms_calculo,
                    x.fim_ms,
                    x.resultado.map(|r| hex(&r)).unwrap_or_default(),
                    texto_json(&x.nota),
                );
            }
        }
        j.push_str("],\"telemetria\":[");
        if let Ok(t) = self.telemetria.lock() {
            for (k, m) in t.iter().enumerate() {
                if k > 0 {
                    j.push(',');
                }
                let _ = write!(
                    j,
                    "{{\"ms\":{},\"tarefa\":{},\"evento\":\"{}\",\"detalhe\":{}}}",
                    m.ms,
                    m.tarefa,
                    m.evento,
                    texto_json(&m.detalhe)
                );
            }
        }
        j.push_str("]}");
        j
    }
}
