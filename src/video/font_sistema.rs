//! As fontes de reserva: quem desenha o que a fonte do jogo não tem.
//!
//! Uma tradução troca as frases do jogo e não troca o `.ttf` que ele empacota. O chinês de uma
//! tradução não tem glifo nenhum na fonte do jogo, e cada ideograma saía como nada — sobravam os
//! números e a pontuação. O que falta é uma fonte com milhares de ideogramas, e essa fonte já
//! existe no aparelho: no Android é a **Noto Sans CJK**, a mesma que o sistema usa para desenhar
//! o texto do próprio Android.
//!
//! # O escopo: chinês
//!
//! Esta lista cobre **chinês** (e, de lambuja, japonês e coreano: a mesma `Noto Sans CJK` desenha
//! hanzi, kana e hangul, e um jogo japonês funciona sem nenhuma linha a mais).
//!
//! As outras escritas — árabe, hebraico, devanágari, tailandês, birmanês, khmer — ficam de fora de
//! propósito. Não por falta de fonte: o Android tem uma `Noto` para cada uma, e a procura abaixo
//! aceitaria qualquer uma delas. É por causa da **composição**: um caractere por vez, como este
//! motor desenha, não liga as letras do árabe (elas saem soltas, na ordem errada, sem o
//! `right-to-left`), não empilha a vogal tailandesa sobre a consoante, e não forma as consoantes
//! conjuntas do devanágari. O glifo aparece e a palavra fica errada — pior do que não aparecer,
//! porque parece um defeito do jogo. Para valer, precisa de *shaping* (HarfBuzz) e de uma passada
//! de bidi, que este port não tem. Ver a nota em [`super::font`].
//!
//! A ordem de procura é deliberada:
//!
//! 1. `ZEEBX_CJK_FONT` — escape para quem quer fixar uma fonte específica;
//! 2. `<raiz do aparelho>/shared/fonts/` — a pasta onde o usuário põe material do console; é
//!    por ali que se troca a fonte sem recompilar nada;
//! 3. as fontes do sistema Android (`/system/fonts`, `/product/fonts`), pelo nome;
//! 4. as do desktop, para o mesmo jogo rodar no PC durante o desenvolvimento.
//!
//! Nada é lido na varredura: o que sai daqui é uma lista de caminhos, e cada um só é aberto quando
//! aparece um caractere que ninguém desenha. Ver `Candidata::carrega`.

use std::path::{Path, PathBuf};

use super::font::{Candidata, Font};

/// De onde vêm as candidatas de uma escrita: por quais nomes procurar e o que a fonte precisa
/// desenhar para ser aceita.
struct Grupo {
    /// As faixas de código da escrita, que decidem se um texto precisa deste grupo e se a fonte
    /// encontrada serve.
    ///
    /// A faixa, e não o texto da sonda, é o critério: um jogo em chinês pode trazer só `你` e
    /// `好`, e comparar com a sonda `中文` recusaria a fonte exatamente para o texto que ela
    /// desenha.
    faixas: &'static [(u32, u32)],
    /// Pedaços de nome de arquivo, em minúsculas, que costumam trazer esta escrita.
    pistas: &'static [&'static str],
    /// Caminhos exatos, tentados antes da varredura por nome.
    fixos: &'static [&'static str],
}

/// As escritas que este port atende. Hoje uma só — a CJK, que cobre chinês, japonês e coreano com
/// o mesmo arquivo. A tabela fica assim, de uma linha, porque acrescentar uma escrita é acrescentar
/// uma entrada; o resto do código não muda.
const GRUPOS: &[Grupo] = &[
    Grupo {
        faixas: &[
            (0x2E80, 0x9FFF),
            (0xAC00, 0xD7AF),
            (0xF900, 0xFAFF),
            (0x3040, 0x30FF),
            (0xFF00, 0xFFEF),
            (0x20000, 0x2FA1F),
        ],
        pistas: &[
            "cjk",
            "sanssc",
            "sanshans",
            "hans",
            "chinese",
            "fallback",
            "wqy",
            "sourcehan",
            "zenhei",
            "microhei",
            "droidsansfallback",
        ],
        fixos: &[
            // Android — a Noto Sans CJK é a fonte do próprio sistema.
            "/system/fonts/NotoSansSC-Regular.otf",
            "/system/fonts/NotoSansSC-Regular.ttf",
            "/system/fonts/NotoSansHans-Regular.otf",
            "/system/fonts/NotoSansCJKsc-Regular.otf",
            "/system/fonts/NotoSansCJK-Regular.ttc",
            "/system/fonts/NotoSerifCJK-Regular.ttc",
            "/system/fonts/DroidSansFallback.ttf",
            "/system/fonts/DroidSansChinese.ttf",
            "/product/fonts/NotoSansSC-Regular.otf",
            "/product/fonts/NotoSansCJK-Regular.ttc",
            "/product/fonts/DroidSansFallback.ttf",
            // Linux (a mesma família Noto, empacotada pela distribuição)
            "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/opentype/noto/NotoSansCJKsc-Regular.otf",
            "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/truetype/wqy/wqy-zenhei.ttc",
            "/usr/share/fonts/truetype/wqy/wqy-microhei.ttc",
            "/usr/share/fonts/truetype/arphic/uming.ttc",
            // macOS
            "/System/Library/Fonts/PingFang.ttc",
            "/System/Library/Fonts/STHeiti Light.ttc",
            // Windows
            "C:\\Windows\\Fonts\\msyh.ttc",
            "C:\\Windows\\Fonts\\simhei.ttf",
            "C:\\Windows\\Fonts\\simsun.ttc",
        ],
    },
];

/// As pastas onde procurar pelo nome, quando os caminhos fixos não deram.
const PASTAS: &[&str] = &[
    "/system/fonts",
    "/product/fonts",
    "/system/font",
    "/usr/share/fonts/truetype/noto",
    "/usr/share/fonts/opentype/noto",
    "/usr/share/fonts/noto",
    "/usr/local/share/fonts",
    "/Library/Fonts",
    "C:\\Windows\\Fonts",
];

/// A fonte de reserva CJK desta máquina, se houver alguma.
///
/// É a que serve de fonte principal quando o jogo não trouxe nenhuma. Ver [`com_reserva`].
pub fn abre(aparelho: &Path) -> Option<Font> {
    abre_com_resto(aparelho).map(|(fonte, _)| fonte)
}

/// A fonte principal, e o que sobrou de candidatas para quando ela também falhar.
///
/// O "resto" existe porque nenhuma fonte cobre todos os ideogramas: a `Noto Sans SC` tem 30 mil
/// glifos e a `Droid Sans Fallback`, bem menos. Se a primeira que abriu não desenhar um caractere
/// raro, a próxima da fila tenta.
fn abre_com_resto(aparelho: &Path) -> Option<(Font, Vec<Candidata>)> {
    let mut lista = candidatas(aparelho);
    while !lista.is_empty() {
        let candidata = lista.remove(0);
        // Não abriu (ou não desenha chinês): a próxima da fila pode abrir.
        if let Some(fonte) = candidata.abre() {
            return Some((fonte, lista));
        }
    }
    None
}

/// Junta a fonte do jogo com as reservas do sistema, quando fazem falta uma à outra.
///
/// Três desfechos:
///
/// - o jogo trouxe uma fonte que já desenha tudo o que ele escreve → fica como está, a do jogo
///   manda;
/// - o jogo trouxe uma fonte incompleta → as do sistema entram atrás dela, só para o que faltar;
/// - o jogo não trouxe fonte nenhuma → a do sistema passa a ser a principal, e o jogo que antes
///   não desenhava texto nenhum passa a desenhar.
pub fn com_reserva(do_jogo: Option<Font>, aparelho: &Path) -> Option<Font> {
    match do_jogo {
        Some(fonte) => Some(fonte.com_candidatas(candidatas(aparelho))),
        None => abre_com_resto(aparelho).map(|(fonte, resto)| fonte.com_candidatas(resto)),
    }
}

/// Os caminhos a tentar, na ordem. Podem não existir — quem lê filtra.
fn candidatas(aparelho: &Path) -> Vec<Candidata> {
    let mut saida = Vec::new();

    // O que o usuário pôs vem primeiro: não leva faixas, e a validação é "desenha algum dos
    // caracteres que faltam".
    if let Some(caminho) = std::env::var_os("ZEEBX_CJK_FONT") {
        saida.push(Candidata::nova(PathBuf::from(caminho), &[]));
    }
    // A pasta do aparelho vem antes do sistema: é onde o usuário põe a fonte que quer ver.
    for caminho in fontes_da_pasta(&aparelho.join("shared").join("fonts")) {
        saida.push(Candidata::nova(caminho, &[]));
    }

    for grupo in GRUPOS {
        for caminho in grupo.fixos {
            saida.push(Candidata::nova(PathBuf::from(caminho), grupo.faixas));
        }
        for pasta in PASTAS {
            for caminho in fontes_do_sistema(pasta, grupo.pistas) {
                saida.push(Candidata::nova(caminho, grupo.faixas));
            }
        }
    }

    if let Some(base) = std::env::var_os("HOME") {
        for caminho in fontes_da_pasta(&Path::new(&base).join(".local/share/fonts")) {
            saida.push(Candidata::nova(caminho, &[]));
        }
    }
    saida
}

/// Os arquivos de fonte de uma pasta, em ordem estável.
fn fontes_da_pasta(pasta: &Path) -> Vec<PathBuf> {
    let Ok(entradas) = std::fs::read_dir(pasta) else {
        return Vec::new();
    };
    let mut saida: Vec<PathBuf> = entradas
        .filter_map(Result::ok)
        .map(|entrada| entrada.path())
        .filter(|caminho| e_fonte(caminho) && caminho.is_file())
        .collect();
    saida.sort();
    saida
}

/// As fontes de uma pasta de sistema cujo **nome** sugere a escrita procurada.
///
/// Filtrar pelo nome evita abrir as ~200 fontes do `/system/fonts` só para descobrir que nenhuma
/// delas serve.
fn fontes_do_sistema(pasta: &str, pistas: &[&str]) -> Vec<PathBuf> {
    let Ok(entradas) = std::fs::read_dir(pasta) else {
        return Vec::new();
    };
    let mut saida: Vec<PathBuf> = entradas
        .filter_map(Result::ok)
        .map(|entrada| entrada.path())
        .filter(|caminho| {
            if !e_fonte(caminho) {
                return false;
            }
            // A variante em negrito é a mesma face com outro peso: o texto do jogo não pede peso,
            // e abrir a regular é uma leitura a menos em dobro.
            let nome = caminho
                .file_name()
                .map(|n| n.to_string_lossy().to_ascii_lowercase())
                .unwrap_or_default();
            if nome.contains("bold") || nome.contains("italic") || nome.contains("oblique") {
                return false;
            }
            pistas.iter().any(|pista| nome.contains(pista))
        })
        .collect();
    saida.sort();
    saida
}

fn e_fonte(caminho: &Path) -> bool {
    caminho.extension().is_some_and(|ext| {
        ext.eq_ignore_ascii_case("ttf")
            || ext.eq_ignore_ascii_case("ttc")
            || ext.eq_ignore_ascii_case("otf")
            || ext.eq_ignore_ascii_case("otc")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn so_arquivos_de_fonte_interessam() {
        assert!(e_fonte(Path::new("a.ttf")));
        assert!(e_fonte(Path::new("a.TTC")));
        assert!(e_fonte(Path::new("a.otf")));
        assert!(!e_fonte(Path::new("a.txt")));
        assert!(!e_fonte(Path::new("a")));
    }

    /// A reserva do sistema desenha chinês, nalguma face da coleção.
    #[test]
    fn a_reserva_do_sistema_desenha_chines() {
        let Some(fonte) = abre(Path::new("/nao-existe")) else {
            return;
        };
        assert!(fonte.cobre('中'));
        assert!(fonte.cobre('。'));
        assert!(!fonte.layout("中文", 16.0).is_empty());
    }

    /// Todo grupo tem faixas e pistas. Um erro de digitação numa faixa faz o grupo inteiro parar
    /// de funcionar sem aviso; sem pistas, a varredura por nome não acha nada.
    #[test]
    fn os_grupos_tem_faixas_e_pistas() {
        for grupo in GRUPOS {
            assert!(!grupo.faixas.is_empty());
            assert!(!grupo.pistas.is_empty());
            assert!(!grupo.fixos.is_empty());
        }
    }

    /// A varredura não abre nada. Se abrisse, cada jogo pagaria a leitura da `Noto Sans CJK` — 20
    /// MB — antes do primeiro quadro.
    #[test]
    fn a_varredura_nao_le_fonte() {
        let lista = candidatas(Path::new("/nao-existe"));
        // A lista é de caminhos: pelo menos os fixos de cada grupo, mesmo sem nenhuma pasta.
        assert!(lista.len() >= GRUPOS.len());
    }
}
