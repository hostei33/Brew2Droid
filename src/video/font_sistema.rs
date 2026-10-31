//! As fontes de reserva: quem desenha o que a fonte do jogo não tem.
//!
//! Uma tradução troca as frases do jogo e não troca o `.ttf` que ele empacota. O chinês de uma
//! tradução não tem glifo nenhum na fonte do jogo, e cada ideograma saía como nada — sobravam os
//! números e a pontuação. O que falta é uma fonte com milhares de ideogramas, e essa fonte já
//! existe no aparelho: no Android é a **Noto Sans CJK**, a mesma que o sistema usa para desenhar
//! o texto do próprio Android.
//!
//! O mesmo vale para as outras escritas, e é por isso que a procura é por **grupo**, não por uma
//! fonte só: a `Noto Sans CJK` desenha latim, cirílico, grego, hanzi, kana e hangul, e não desenha
//! um único caractere de árabe, hebraico, devanágari, tailandês, birmanês ou khmer. O Android tem
//! uma `Noto` separada para cada uma delas, e é essa que se procura quando o texto pede.
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

/// Um grupo de escrita: o texto que o identifica e por onde procurá-lo.
struct Grupo {
    /// O que a fonte precisa saber desenhar. Serve para decidir se vale abrir o arquivo e para
    /// recusar um `.ttf` de nome prometedor que não cubra o grupo.
    sonda: &'static str,
    /// As faixas de código da escrita, que decidem se um texto precisa deste grupo.
    ///
    /// Não dá para usar a sonda para isso: `ไทย` e `สวัสดี` são tailandês e não têm uma letra em
    /// comum — comparar texto recusaria a fonte exatamente para o texto que ela desenha.
    faixas: &'static [(u32, u32)],
    /// Pedaços de nome de arquivo, em minúsculas, que costumam trazer esta escrita.
    pistas: &'static [&'static str],
    /// Caminhos exatos, tentados antes da varredura por nome.
    fixos: &'static [&'static str],
}

/// As escritas que o Android traz e um jogo pode acabar usando.
///
/// A ordem importa: a CJK vem primeiro porque é a que cobre mais de uma escrita de uma vez (latim,
/// grego, cirílico, hanzi, kana, hangul), então ela resolve a maioria dos casos com uma leitura só.
const GRUPOS: &[Grupo] = &[
    Grupo {
        sonda: "中文",
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
    Grupo {
        sonda: "مرحبا",
        faixas: &[
            (0x0600, 0x06FF),
            (0x0750, 0x077F),
            (0x08A0, 0x08FF),
            (0xFB50, 0xFDFF),
            (0xFE70, 0xFEFF),
        ],
        pistas: &["arabic", "naskh", "kufi", "urdu"],
        fixos: &[
            "/system/fonts/NotoNaskhArabic-Regular.ttf",
            "/system/fonts/NotoSansArabic-Regular.ttf",
            "/usr/share/fonts/truetype/noto/NotoSansArabic-Regular.ttf",
        ],
    },
    Grupo {
        sonda: "שלום",
        faixas: &[(0x0590, 0x05FF), (0xFB1D, 0xFB4F)],
        pistas: &["hebrew"],
        fixos: &[
            "/system/fonts/NotoSansHebrew-Regular.ttf",
            "/usr/share/fonts/truetype/noto/NotoSansHebrew-Regular.ttf",
        ],
    },
    Grupo {
        sonda: "नमस्ते",
        faixas: &[(0x0900, 0x097F), (0xA8E0, 0xA8FF)],
        pistas: &["devanagari"],
        fixos: &[
            "/system/fonts/NotoSansDevanagari-Regular.ttf",
            "/usr/share/fonts/truetype/noto/NotoSansDevanagari-Regular.ttf",
        ],
    },
    Grupo {
        sonda: "বাংলা",
        faixas: &[(0x0980, 0x09FF)],
        pistas: &["bengali"],
        fixos: &[
            "/system/fonts/NotoSansBengali-Regular.ttf",
            "/usr/share/fonts/truetype/noto/NotoSansBengali-Regular.ttf",
        ],
    },
    Grupo {
        sonda: "தமிழ்",
        faixas: &[(0x0B80, 0x0BFF)],
        pistas: &["tamil"],
        fixos: &[
            "/system/fonts/NotoSansTamil-Regular.ttf",
            "/usr/share/fonts/truetype/noto/NotoSansTamil-Regular.ttf",
        ],
    },
    Grupo {
        sonda: "తెలుగు",
        faixas: &[(0x0C00, 0x0C7F)],
        pistas: &["telugu"],
        fixos: &[
            "/system/fonts/NotoSansTelugu-Regular.ttf",
            "/usr/share/fonts/truetype/noto/NotoSansTelugu-Regular.ttf",
        ],
    },
    Grupo {
        sonda: "ಕನ್ನಡ",
        faixas: &[(0x0C80, 0x0CFF)],
        pistas: &["kannada"],
        fixos: &[
            "/system/fonts/NotoSansKannada-Regular.ttf",
            "/usr/share/fonts/truetype/noto/NotoSansKannada-Regular.ttf",
        ],
    },
    Grupo {
        sonda: "മലയാളം",
        faixas: &[(0x0D00, 0x0D7F)],
        pistas: &["malayalam"],
        fixos: &[
            "/system/fonts/NotoSansMalayalam-Regular.ttf",
            "/usr/share/fonts/truetype/noto/NotoSansMalayalam-Regular.ttf",
        ],
    },
    Grupo {
        sonda: "ગુજરાતી",
        faixas: &[(0x0A80, 0x0AFF)],
        pistas: &["gujarati"],
        fixos: &[
            "/system/fonts/NotoSansGujarati-Regular.ttf",
            "/usr/share/fonts/truetype/noto/NotoSansGujarati-Regular.ttf",
        ],
    },
    Grupo {
        sonda: "ਪੰਜਾਬੀ",
        faixas: &[(0x0A00, 0x0A7F)],
        pistas: &["gurmukhi"],
        fixos: &[
            "/system/fonts/NotoSansGurmukhi-Regular.ttf",
            "/usr/share/fonts/truetype/noto/NotoSansGurmukhi-Regular.ttf",
        ],
    },
    Grupo {
        sonda: "ଓଡ଼ିଆ",
        faixas: &[(0x0B00, 0x0B7F)],
        pistas: &["oriya", "odia"],
        fixos: &[
            "/system/fonts/NotoSansOriya-Regular.ttf",
            "/usr/share/fonts/truetype/noto/NotoSansOriya-Regular.ttf",
        ],
    },
    Grupo {
        sonda: "සිංහල",
        faixas: &[(0x0D80, 0x0DFF)],
        pistas: &["sinhala"],
        fixos: &[
            "/system/fonts/NotoSansSinhala-Regular.ttf",
            "/usr/share/fonts/truetype/noto/NotoSansSinhala-Regular.ttf",
        ],
    },
    Grupo {
        sonda: "ไทย",
        faixas: &[(0x0E00, 0x0E7F)],
        pistas: &["thai"],
        fixos: &[
            "/system/fonts/NotoSansThai-Regular.ttf",
            "/usr/share/fonts/truetype/noto/NotoSansThai-Regular.ttf",
        ],
    },
    Grupo {
        sonda: "ລາວ",
        faixas: &[(0x0E80, 0x0EFF)],
        pistas: &["lao"],
        fixos: &[
            "/system/fonts/NotoSansLao-Regular.ttf",
            "/usr/share/fonts/truetype/noto/NotoSansLao-Regular.ttf",
        ],
    },
    Grupo {
        sonda: "မြန်မာ",
        faixas: &[(0x1000, 0x109F), (0xAA60, 0xAA7F)],
        pistas: &["myanmar"],
        fixos: &[
            "/system/fonts/NotoSansMyanmar-Regular.ttf",
            "/usr/share/fonts/truetype/noto/NotoSansMyanmar-Regular.ttf",
        ],
    },
    Grupo {
        sonda: "ខ្មែរ",
        faixas: &[(0x1780, 0x17FF), (0x19E0, 0x19FF)],
        pistas: &["khmer"],
        fixos: &[
            "/system/fonts/NotoSansKhmer-Regular.ttf",
            "/usr/share/fonts/truetype/noto/NotoSansKhmer-Regular.ttf",
        ],
    },
    Grupo {
        sonda: "አማርኛ",
        faixas: &[(0x1200, 0x137F), (0x1380, 0x139F), (0x2D80, 0x2DDF)],
        pistas: &["ethiopic"],
        fixos: &[
            "/system/fonts/NotoSansEthiopic-Regular.ttf",
            "/usr/share/fonts/truetype/noto/NotoSansEthiopic-Regular.ttf",
        ],
    },
    Grupo {
        sonda: "հայերեն",
        faixas: &[(0x0530, 0x058F), (0xFB13, 0xFB17)],
        pistas: &["armenian"],
        fixos: &[
            "/system/fonts/NotoSansArmenian-Regular.ttf",
            "/usr/share/fonts/truetype/noto/NotoSansArmenian-Regular.ttf",
        ],
    },
    Grupo {
        sonda: "ქართული",
        faixas: &[(0x10A0, 0x10FF), (0x1C90, 0x1CBF), (0x2D00, 0x2D2F)],
        pistas: &["georgian"],
        fixos: &[
            "/system/fonts/NotoSansGeorgian-Regular.ttf",
            "/usr/share/fonts/truetype/noto/NotoSansGeorgian-Regular.ttf",
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

/// A fonte principal, e o que sobrou de candidatas para as outras escritas.
///
/// Sem o resto, um jogo que não trouxe `.ttf` desenharia chinês e nada mais: a `Noto Sans CJK`
/// cobre o chinês e não cobre árabe, tailandês nem devanágari.
fn abre_com_resto(aparelho: &Path) -> Option<(Font, Vec<Candidata>)> {
    let mut lista = candidatas(aparelho);
    loop {
        let posicao = lista.iter().position(Candidata::e_cjk)?;
        let candidata = lista.remove(posicao);
        // Não abriu: a próxima que sirva para CJK pode abrir.
        if let Some(fonte) = candidata.abre() {
            return Some((fonte, lista));
        }
    }
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

    // O que o usuário pôs vem primeiro e vale para qualquer escrita: não leva sonda, e a validação
    // é "desenha algum dos caracteres que faltam".
    if let Some(caminho) = std::env::var_os("ZEEBX_CJK_FONT") {
        saida.push(Candidata::nova(PathBuf::from(caminho), "", &[]));
    }
    // A pasta do aparelho vem antes do sistema: é onde o usuário põe a fonte que quer ver.
    for caminho in fontes_da_pasta(&aparelho.join("shared").join("fonts")) {
        saida.push(Candidata::nova(caminho, "", &[]));
    }

    for grupo in GRUPOS {
        for caminho in grupo.fixos {
            saida.push(Candidata::nova(
                PathBuf::from(caminho),
                grupo.sonda,
                grupo.faixas,
            ));
        }
        for pasta in PASTAS {
            for caminho in fontes_do_sistema(pasta, grupo.pistas) {
                saida.push(Candidata::nova(caminho, grupo.sonda, grupo.faixas));
            }
        }
    }

    if let Some(base) = std::env::var_os("HOME") {
        for caminho in fontes_da_pasta(&Path::new(&base).join(".local/share/fonts")) {
            saida.push(Candidata::nova(caminho, "", &[]));
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

    /// Nenhuma das sondas pode ser vazia, e cada uma tem de ser coberta por uma fonte que os
    /// nomes de arquivo daquele grupo possam trazer. Erro de digitação numa sonda faz o grupo
    /// inteiro parar de funcionar sem aviso.
    #[test]
    fn as_sondas_sao_plausiveis() {
        for grupo in GRUPOS {
            assert!(!grupo.sonda.is_empty());
            assert!(!grupo.pistas.is_empty(), "{}", grupo.sonda);
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
