<!--
Transcrição fiel do documento do autor "HYURAX / ULTRAX — Documento Mestre do
Projeto" (HYURAX_ULTRAX_Documento_Mestre.docx), recebido em 02/10/2026.
É a referência de prioridades do projeto. A auditoria contra ele está em
AUDITORIA-DOCUMENTO-MESTRE.md.
-->

# HYURAX / ULTRAX — Documento Mestre do Projeto

Visão, arquitetura, infraestrutura, economia, segurança e roadmap.
Versão de referência para desenvolvimento e auditoria técnica.

## 1. Visão geral

HYURAX / ULTRAX é uma proposta de infraestrutura de computação distribuída que conecta quem precisa de capacidade computacional a máquinas capazes de executar trabalhos úteis. A plataforma pretende receber tarefas, distribuir unidades computacionais, executar os trabalhos, acompanhar a execução, verificar resultados e entregar dados e relatórios ao cliente.

O projeto não deve ser tratado simplesmente como uma criptomoeda, um minerador tradicional, uma carteira ou um SaaS convencional. A visão combina computação distribuída, computação científica, infraestrutura como serviço, verificação computacional, marketplace, aplicações web e móveis e uma camada econômica baseada no ativo HYX.

Fluxo conceitual: Demanda computacional → distribuição → execução → verificação → resultados → liquidação.

## 2. Objetivo do projeto

Construir uma infraestrutura tecnológica capaz de transformar capacidade computacional disponível em um serviço mensurável, verificável e comercializável, permitindo que universidades, laboratórios, pesquisadores, empresas e outros clientes utilizem recursos computacionais sem precisar possuir toda a infraestrutura necessária.

## 3. Núcleo computacional para PC

O programa para PC é o componente central da rede. Ele deve participar da execução real dos trabalhos e funcionar de forma independente da interface gráfica.

- Registro e identidade do dispositivo.
- Autenticação e comunicação segura.
- Recebimento e validação de tarefas.
- Gerenciamento de work units.
- Execução por CPU/GPU conforme compatibilidade.
- Checkpoints e recuperação de tarefas.
- Envio e recebimento de resultados.
- Verificação e evidências de execução.
- Telemetria real de hardware.
- Logs, tratamento de falhas e reconexão.
- Atualizações seguras e assinadas.
- Controle de recursos e preferências do usuário.

Nenhuma tarefa remota deve receber acesso irrestrito ao computador. A execução precisa ser isolada, com validação de entradas, limites de recursos e proteção contra execução arbitrária de código.

## 4. Computação científica distribuída

O sistema poderá atender trabalhos em áreas como:

- Biologia molecular e simulações.
- Genética computacional e simulação de populações.
- Agricultura.
- Ciência dos materiais.
- Energia.
- Meio ambiente.
- Matemática e física.
- Economia e logística.
- Inteligência artificial e aprendizado de máquina.
- Otimização e outros problemas computacionais.

A plataforma deve distinguir claramente cálculo computacional de validação científica. Uma saída calculada ou matematicamente consistente não é automaticamente uma descoberta científica validada.

## 5. Distribuição, escalabilidade e checkpoints

- Divisão de trabalhos em lotes e unidades computacionais.
- Filas e agendamento.
- Seleção de nós compatíveis.
- Balanceamento de carga.
- Persistência de estado.
- Retomada após interrupções.
- Reexecução seletiva.
- Controle de duplicidade.
- Tolerância a nós desconectados.
- Reprodutibilidade e registro de parâmetros.

## 6. Verificação computacional

A confiança nos resultados é um dos pilares do projeto. A verificação pode ocorrer em diferentes níveis:

- Integridade: hashes, assinaturas e integridade dos dados.
- Reexecução: repetir uma unidade de trabalho independentemente.
- Múltiplos nós: comparar resultados de participantes diferentes.
- Verificação independente: utilizar validadores independentes quando necessário.
- Reprodutibilidade: registrar parâmetros, versões, dependências e evidências necessárias para repetir o trabalho.

Métodos especializados, como verificações probabilísticas de determinadas operações matemáticas, devem ser utilizados apenas quando forem adequados ao algoritmo. Não existe um verificador universal para qualquer trabalho científico.

## 7. Telemetria e visualização 3D

O projeto pretende possuir uma visualização 3D baseada em eventos reais. A interface não deve simular atividade computacional que não esteja ocorrendo.

- ID do job, work unit e nó.
- Tipo de tarefa e estado.
- Progresso real.
- CPU, GPU, VRAM e RAM.
- Temperatura e energia quando disponível.
- Throughput.
- Checkpoints e hashes.
- Tentativas, falhas e reexecuções.
- Resultado da verificação.
- Histórico e reprodução temporal.
- Inspeção individual de computações e nós.

Quando uma métrica não estiver disponível no hardware, ela deve ser marcada como indisponível, e não substituída por um valor fictício.

## 8. Economia e HYX

HYX é o ativo econômico planejado para o ecossistema. A proposta é relacionar sua utilidade à atividade da rede, sem assumir que o uso da plataforma garante valorização do ativo.

- Pagamentos e liquidação conforme a arquitetura definida.
- Taxas da plataforma e da rede.
- Recompensas de operadores quando aplicáveis.
- Marketplace de capacidade computacional.
- Créditos de computação separados quando fizer sentido.
- Contabilidade e histórico de operações.
- Possível integração com um índice econômico próprio.

O projeto deve explicar por que o HYX é necessário em cada função. Não deve criar demanda artificial por um token quando uma solução mais simples e segura atender ao mesmo objetivo.

## 9. HYX Index

Existe a proposta de um índice econômico associado a diferentes mercados e ativos, inicialmente considerando categorias como Bitcoin, ouro, prata, paládio e petróleo, entre outras que ainda precisam ser definidas.

O índice precisa ter metodologia clara. Acompanhar o preço de ativos não significa automaticamente que o HYX seja lastreado nesses ativos. Qualquer exposição financeira, custódia, resgate ou promessa de lastro exige desenho jurídico, financeiro, operacional e regulatório específico.

## 10. Marketplace de computação

- Cadastro de operadores e máquinas.
- Capacidade e compatibilidade verificadas.
- Disponibilidade e preços.
- Estimativas de duração e custo.
- Execução e verificação.
- Reputação e histórico.
- Pagamentos.
- Mecanismos de resolução de disputas.
- Detecção de comportamento malicioso.

O sistema não deve prometer rentabilidade garantida aos operadores. Custos de energia, hardware, conectividade e tempo de execução precisam ser considerados.

## 11. Nós verificados e reputação

Existe a ideia de classificar nós com histórico consistente de operação como nós verificados, usando três meses como referência inicial. A classificação deve ser baseada em evidências mensuráveis, e não apenas no tempo de cadastro.

O sistema de reputação/Gold Score deve ser documentado e projetado para reduzir manipulação e identidades falsas.

## 12. Segurança e identidade

- Identidade criptográfica dos nós.
- Autenticação e autorização.
- Rotação e revogação de credenciais.
- Proteção de chaves privadas.
- Proteção contra replay.
- Rate limiting.
- Detecção de Sybil.
- Integridade de executáveis.
- Atualizações assinadas.
- Comunicação autenticada.
- Isolamento de tarefas.
- Logs e resposta a incidentes.

Nenhum mecanismo isolado, como IP, identificador de dispositivo ou hash, deve ser tratado como solução completa contra Sybil ou comprometimento de máquinas.

## 13. Blockchain e carteira

O projeto possui conceitos relacionados a carteira, chaves criptográficas, saldos, nonces, transações, Proof of Work e validações. Esses componentes precisam ser auditados antes de qualquer afirmação de prontidão para produção.

- Carteira e gestão de chaves.
- Assinatura e validação de transações.
- Estado contábil.
- Consenso.
- Emissão.
- Registro de blocos.
- Validação de trabalhos.
- Recompensas.

A execução de um trabalho científico e o consenso da blockchain são problemas diferentes. Eles devem permanecer separados na arquitetura, salvo quando uma integração específica for comprovadamente necessária.

## 14. Modelo OaaS, Compute-as-a-Service e SaaS

A camada comercial do projeto pode combinar diferentes modelos. OaaS (Operations as a Service) descreve a entrega de uma operação computacional completa como serviço. Compute-as-a-Service descreve a oferta de capacidade computacional. SaaS descreve o software e os painéis utilizados para acessar e administrar os serviços.

A experiência conceitual é:

Cliente → envia tarefa → Ultrax coordena → máquinas executam → rede verifica → cliente recebe resultado.

- Cobrança por computação.
- Assinaturas.
- Planos empresariais.
- API.
- Recursos premium.
- Marketplace de capacidade.

## 15. Programa para PC — prioridade atual

A prioridade atual é finalizar o programa central para PC antes de avançar significativamente no aplicativo móvel.

- Auditar o núcleo existente.
- Corrigir falhas.
- Validar execução real.
- Testar tarefas longas.
- Melhorar recuperação.
- Implementar e revisar segurança.
- Melhorar telemetria.
- Aprimorar interface e experiência.
- Garantir atualização segura.

A interface gráfica não deve conter a lógica crítica do sistema. Execução, comunicação, armazenamento, telemetria e UI devem possuir responsabilidades bem delimitadas.

## 16. Aplicativo móvel

O aplicativo será desenvolvido depois que o núcleo para PC estiver estável. O celular funcionará principalmente como interface de controle, carteira, marketplace e acompanhamento.

- Carteira.
- Envio e recebimento conforme protocolo.
- Acompanhamento de tarefas.
- Envio de trabalhos.
- Consulta de resultados.
- Aluguel/contratação de máquinas.
- Marketplace.
- Notificações.
- Administração da conta.

## 17. Dashboard web

A experiência do cliente deverá permitir:

- Criar e administrar conta.
- Escolher serviços.
- Enviar tarefas.
- Configurar parâmetros.
- Acompanhar processamento.
- Consultar custos e estado.
- Visualizar resultados.
- Baixar relatórios.
- Consultar histórico.
- Administrar pagamentos e créditos.

O instalador para PC deve ser distribuído por canal confiável e utilizar mecanismos de integridade, assinatura e atualização segura.

## 18. Relatórios

- JSON.
- CSV.
- PDF.
- Dados brutos quando aplicável.
- Metadados.
- Parâmetros de execução.
- Versões dos algoritmos.
- Evidências de integridade.
- Métricas.
- Histórico.
- Informações de verificação.

## 19. Arquitetura de referência

- Cliente de PC — execução, recursos, telemetria e comunicação.
- Camada de tarefas — criação, validação, divisão e distribuição.
- Rede de nós — registro, disponibilidade e coordenação.
- Camada de verificação — integridade, reexecução e reprodutibilidade.
- Camada científica — algoritmos, bibliotecas e ambientes.
- Camada econômica — créditos, pagamentos, taxas e recompensas.
- Blockchain/carteira — transações, chaves e consenso.
- Marketplace — oferta, demanda, preços e contratação.
- Backend/API — autenticação, autorização, persistência e integração.
- Dashboard web — clientes e administração.
- Aplicativo móvel — carteira, controle e marketplace.
- Telemetria/visualização — eventos reais e histórico.
- Segurança/operações — monitoramento, auditoria e atualização.

## 20. Estado conhecido do projeto

Já existem componentes e testes relacionados a tipos de trabalho, núcleo de Proof of Work, carteira, registro de dispositivos, P2P simulado, marketplace de tarefas, mecanismos de validação, comunicação entre dispositivos, execução prolongada e interface web. O estado exato de cada componente deve ser confirmado por inspeção do repositório e execução de testes.

Nenhuma funcionalidade deve ser considerada pronta para produção apenas porque possui uma interface. É necessário distinguir implementação real, teste, simulação, protótipo e funcionalidade incompleta.

## 21. Roadmap

| Fase | Itens |
|---|---|
| 1 — Auditoria | Mapear arquitetura · Identificar módulos reais e simulados · Revisar dependências · Executar testes · Documentar falhas |
| 2 — Núcleo PC | Estabilizar execução · Checkpoints · Recuperação · Telemetria · Comunicação · Logs |
| 3 — Cibersegurança | Modelo de ameaças · Isolamento · Autenticação · Autorização · Chaves · Atualizações assinadas · Testes de segurança |
| 4 — Design | Dashboard · Gráficos · Histórico · Configurações · Estados de erro · Acessibilidade |
| 5 — Backend | APIs · Contas · Tarefas · Coordenação · Resultados · Relatórios |
| 6 — Rede | Múltiplos nós · Verificação · Falhas · Carga · Consistência |
| 7 — Cliente | Dashboard · Instalador · Acompanhamento · Resultados · Custos |
| 8 — Mobile | Carteira · Marketplace · Tarefas · Resultados · Notificações |
| 9 — Economia | Cobrança · Créditos · Recompensas · Tokenomics · Liquidez · Regulação |
| 10 — Produção | Auditorias · Monitoramento · Disaster recovery · Documentação · Implantação gradual |

## 22. Instruções para desenvolvimento

Antes de alterar grandes partes do sistema, o desenvolvedor deve entender o código existente. Não substituir implementações reais por mocks, animações ou simulações sem autorização explícita.

Para cada funcionalidade, registrar:

- O que já existe.
- O que realmente funciona.
- Como foi testado.
- Quais riscos existem.
- O que está incompleto.
- O que precisa ser corrigido.
- Como comprovar a conclusão.

Prioridades: correção, segurança, observabilidade, modularidade e desempenho mensurável. Evitar promessas de escalabilidade, descentralização, rentabilidade, segurança ou precisão científica sem evidências.

## 23. Objetivo final

Construir uma infraestrutura tecnológica que conecte demanda computacional a capacidade computacional disponível, execute trabalhos úteis, verifique resultados e entregue serviços de maneira segura, observável e economicamente sustentável.

A prioridade imediata é estabilizar, auditar, proteger e aprimorar o programa central para PC. Somente depois disso o desenvolvimento do aplicativo móvel e a expansão completa do ecossistema devem avançar.
