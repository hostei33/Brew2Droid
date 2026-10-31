//! A fonte de reserva: quem desenha o que a fonte do jogo não tem.
//!
//! Uma tradução troca as frases do jogo e não troca o `.ttf` que ele empacota. O chinês de uma
//! tradução não tem glifo nenhum na fonte do jogo, e cada ideograma saía como nada — sobravam os
//! números e a pontuação. O que falta é uma fonte com milhares de ideogramas, e essa fonte já
//! existe no aparelho: no Android é a **Noto Sans CJK**, a mesma que o sistema usa para desenhar
//! o texto do próprio Android.
//!
//! A ordem de procura é deliberada:
//!
//! 1. `ZEEBX_CJK_FONT` — escape para quem quer fixar uma fonte específica;
//! 2. `<raiz do aparelho>/shared/fonts/` — a pasta onde o usuário põe material do console; é
//!    por ali que se troca a fonte sem recompilar nada;
//! 3. as fontes do sistema Android (`/system/fonts`, `/product/fonts`), pelo nome — Noto Sans
//!    SC/CJK, Droid Sans Fallback, e o que o fabricante tiver posto;
//! 4. as do desktop, para o mesmo jogo rodar no PC durante o desenvolvimento.
//!
//! O arquivo só é aceito se a face escolhida **desenhar** `中` e `文`: um `.ttf` de nome
//! promissor que não cubra ideogramas seria pior do que nenhum, porque desligaria a procura.

use std::path::{Path, PathBuf};

use super::font::Font;

/// A reserva desta máquina, se houver alguma.
///
/// A varredura é feita uma vez por jogo aberto, e não por quadro: são alguns `stat` e, no pior
/// caso, um `read` do arquivo escolhido.
pub fn abre(aparelho: &Path) -> Option<Font> {
    for caminho in candidatos(aparelho) {
        if !caminho.is_file() {
            continue;
        }
        let Ok(bytes) = std::fs::read(&caminho) else {
            continue;
        };
        let tamanho = bytes.len();
        let indice = indice_da_face(&bytes);
        let Some(fonte) = Font::load_index(bytes, indice, nome_do(&caminho)) else {
            continue;
        };
        if !(fonte.cobre('中') && fonte.cobre('文')) {
            continue;
        }
        crate::registro!(
            crate::registro::Nivel::Informacao,
            "texto",
            "fonte de reserva: {} (face {indice}, {} KiB)",
            caminho.display(),
            tamanho / 1024
        );
        return Some(fonte);
    }
    None
}

/// Junta a fonte do jogo com a reserva do sistema, quando as duas fazem falta uma à outra.
///
/// Três desfechos:
///
/// - o jogo trouxe uma fonte que já desenha CJK → fica como está, a do jogo manda;
/// - o jogo trouxe uma fonte sem CJK → a do sistema entra atrás dela, só para os ideogramas;
/// - o jogo não trouxe fonte nenhuma → a do sistema passa a ser a principal, e o jogo que antes
///   não desenhava texto nenhum passa a desenhar.
pub fn com_reserva(do_jogo: Option<Font>, aparelho: &Path) -> Option<Font> {
    let Some(do_jogo) = do_jogo else {
        return abre(aparelho);
    };
    if do_jogo.cobre('中') {
        return Some(do_jogo);
    }
    match abre(aparelho) {
        Some(reserva) => Some(do_jogo.com_reserva(reserva)),
        None => {
            crate::registro!(
                crate::registro::Nivel::Aviso,
                "texto",
                "a fonte {} não desenha chinês e não há fonte de reserva no sistema",
                do_jogo.source
            );
            Some(do_jogo)
        }
    }
}

/// Os caminhos a tentar, na ordem. Podem não existir — quem chama filtra.
fn candidatos(aparelho: &Path) -> Vec<PathBuf> {
    let mut saida = Vec::new();
    if let Some(caminho) = std::env::var_os("ZEEBX_CJK_FONT") {
        saida.push(PathBuf::from(caminho));
    }
    // A pasta do aparelho vem antes do sistema: é onde o usuário põe a fonte que quer ver.
    saida.extend(fontes_da_pasta(&aparelho.join("shared").join("fonts")));

    for caminho in DO_SISTEMA {
        saida.push(PathBuf::from(caminho));
    }
    // O nome do arquivo muda de fabricante para fabricante (`NotoSansCJK-Regular.ttc`,
    // `DroidSansFallback.ttf`, `NotoSansSC-Regular.otf`), então além da lista fixa vale olhar a
    // pasta inteira pelo nome.
    for pasta in ["/system/fonts", "/product/fonts", "/system/font"] {
        saida.extend(fontes_do_sistema(pasta));
    }
    if let Some(base) = std::env::var_os("HOME") {
        saida.extend(fontes_da_pasta(
            &Path::new(&base).join(".local/share/fonts"),
        ));
    }
    saida
}

/// As do Android (as do Google, que é o que o sistema usa) e, depois, as do desktop.
const DO_SISTEMA: &[&str] = &[
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
];

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

/// As fontes de uma pasta de sistema cujo **nome** sugere CJK.
///
/// Filtrar pelo nome evita abrir as ~200 fontes do `/system/fonts` só para descobrir que nenhuma
/// delas cobre ideograma.
fn fontes_do_sistema(pasta: &str) -> Vec<PathBuf> {
    const PISTAS: &[&str] = &[
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
    ];
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
            let nome = caminho
                .file_name()
                .map(|n| n.to_string_lossy().to_ascii_lowercase())
                .unwrap_or_default();
            PISTAS.iter().any(|pista| nome.contains(pista))
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

fn nome_do(caminho: &Path) -> String {
    caminho
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| caminho.display().to_string())
}

/// Qual face abrir dentro do arquivo.
///
/// Um `.ttf`/`.otf` tem uma só, e a resposta é zero. Uma coleção (`.ttc`) tem várias, e a
/// `NotoSansCJK-Regular.ttc` — a que o Android e as distribuições Linux trazem — põe JP, KR, SC,
/// TC e HK no mesmo arquivo. Abrir a primeira daria o japonês para um texto em chinês: os
/// ideogramas aparecem, mas com formas que num texto chinês se leem errado. Então procura-se a
/// face cujo nome diz "SC", "Hans" ou "Simplified".
fn indice_da_face(dados: &[u8]) -> u32 {
    if dados.get(..4) != Some(b"ttcf") {
        return 0;
    }
    let Some(quantas) = ttf_parser::fonts_in_collection(dados) else {
        return 0;
    };
    for indice in 0..quantas {
        let Ok(face) = ttf_parser::Face::parse(dados, indice) else {
            continue;
        };
        if e_simplificado(&face) {
            return indice;
        }
    }
    0
}

/// Se o nome da face diz que ela é a variante de chinês simplificado.
///
/// Compara **palavra inteira** e só na família: o nome PostScript de toda face desta coleção
/// começa com `NotoSansCJK`, e `NotoSansCJKjp` contém "SC" — procurar o pedaço de texto daria
/// positivo em todas as cinco.
fn e_simplificado(face: &ttf_parser::Face) -> bool {
    /// `FAMILY` e `TYPOGRAPHIC_FAMILY`.
    const NOMES: [u16; 2] = [1, 16];
    face.names()
        .into_iter()
        .filter(|nome| NOMES.contains(&nome.name_id))
        .filter_map(|nome| nome.to_string())
        .any(|nome| {
            nome.split(|c: char| !c.is_alphanumeric())
                .map(|palavra| palavra.to_ascii_uppercase())
                .any(|palavra| {
                    matches!(
                        palavra.as_str(),
                        "SC" | "HANS" | "SIMPLIFIED" | "GB2312" | "CN"
                    )
                })
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

    #[test]
    fn um_ttf_simples_e_a_face_zero() {
        assert_eq!(indice_da_face(b"nao sou uma fonte"), 0);
    }

    /// A coleção do sistema, quando existe nesta máquina. É a mesma família que o Android usa.
    fn colecao() -> Option<Vec<u8>> {
        ["/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc"]
            .iter()
            .find_map(|caminho| std::fs::read(caminho).ok())
    }

    #[test]
    fn a_colecao_abre_na_face_do_chines_simplificado() {
        let Some(dados) = colecao() else {
            return;
        };
        let indice = indice_da_face(&dados);
        assert_ne!(
            indice, 0,
            "a coleção devia abrir numa face que não a japonesa"
        );
        let face = ttf_parser::Face::parse(&dados, indice).unwrap();
        let nome = face
            .names()
            .into_iter()
            .filter(|n| n.name_id == 4)
            .filter_map(|n| n.to_string())
            .next()
            .unwrap_or_default();
        assert!(
            nome.to_ascii_uppercase().contains("SC"),
            "face escolhida não é a simplificada: {nome}"
        );
    }

    #[test]
    fn a_reserva_do_sistema_desenha_chines() {
        let Some(fonte) = abre(Path::new("/nao-existe")) else {
            return;
        };
        assert!(fonte.cobre('中'));
        assert!(fonte.cobre('。'));
        assert!(!fonte.layout("中文", 16.0).is_empty());
    }
}
