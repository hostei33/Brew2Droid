//! O texto que os jogos mandam desenhar.
//!
//! Até aqui o `IDISPLAY_DrawText` recebia a frase certa e não tinha com o que desenhá-la: o
//! texto ia para uma lista que só aparecia no relatório. Isso é o suficiente para saber que um
//! jogo *quer* escrever, e insuficiente para qualquer tela com menu.
//!
//! A fonte do console vinha da firmware, que não temos. Mas vários módulos trazem a própria: a
//! Z-Wheel empacota uma `tectoy.ttf` de 191 KB, e é a fonte com que a loja foi desenhada. Então
//! a regra é: **usar a fonte que veio com o jogo**. Quando não há nenhuma, continua sem texto —
//! e o relatório diz isso, em vez de desenhar com uma fonte que não é a dele.
//!
//! # A reserva
//!
//! A fonte do jogo desenha o que o jogo escreve no idioma para o qual foi feito. Uma tradução
//! troca as frases e não troca o `.ttf`: o chinês de uma tradução não tem glifo nenhum na
//! `tectoy.ttf`, e cada ideograma saía como nada — a frase ficava só com os números.
//!
//! Por isso a fonte do jogo pode vir acompanhada de uma **reserva**, consultada apenas caractere
//! a caractere, e só quando a principal devolve `.notdef`. A reserva não substitui a fonte do
//! jogo (o latim continua saindo com as formas dele, e as métricas da linha também): ela só
//! preenche o que faltaria. Quem escolhe a reserva é o [`super::font_sistema`].

use ab_glyph::{Font as _, FontVec, GlyphId, PxScale, ScaleFont as _};

/// Uma fonte carregada de um `.ttf` do próprio jogo.
pub struct Font {
    face: FontVec,
    /// A fonte do sistema, para os caracteres que a do jogo não tem.
    reserva: Option<FontVec>,
    /// O ajuste de tamanho da reserva, para ela ter o mesmo `em` da principal.
    escala_reserva: f32,
    /// De onde ela veio, para o relatório.
    pub source: String,
    /// De onde veio a reserva, quando há uma.
    pub reserve_source: Option<String>,
}

/// Um glifo já rasterizado: onde pôr e com que cobertura.
pub struct Glyph {
    /// Deslocamento em relação ao ponto de desenho, já com o `bearing` aplicado.
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    /// Cobertura por pixel, de 0 a 255, na ordem de leitura.
    pub coverage: Vec<u8>,
}

impl Font {
    /// Lê um `.ttf`. `None` quando o arquivo não é uma fonte que saibamos abrir.
    pub fn load(bytes: Vec<u8>, source: String) -> Option<Self> {
        Self::load_index(bytes, 0, source)
    }

    /// Lê a face `index` de um `.ttf` ou `.ttc`.
    ///
    /// As coleções (`.ttc`) empacotam várias faces no mesmo arquivo — a `NotoSansCJK-Regular.ttc`
    /// traz JP, KR, SC, TC e HK. Abrir sempre a zero daria o japonês para um texto em chinês: os
    /// ideogramas aparecem, mas com as formas japonesas, que num texto em chinês se leem errado.
    pub fn load_index(bytes: Vec<u8>, index: u32, source: String) -> Option<Self> {
        FontVec::try_from_vec_and_index(bytes, index)
            .ok()
            .map(|face| Self {
                face,
                reserva: None,
                escala_reserva: 1.0,
                source,
                reserve_source: None,
            })
    }

    /// Põe `reserva` atrás desta fonte, para os caracteres que ela não desenha.
    ///
    /// Quando a principal já cobre CJK — um jogo chinês que trouxe a própria fonte —, a reserva
    /// é descartada: a fonte do jogo sempre ganha.
    pub fn com_reserva(mut self, reserva: Font) -> Self {
        if self.cobre('中') {
            return self;
        }
        self.escala_reserva = escala_para_em(&self.face, &reserva.face);
        self.reserva = Some(reserva.face);
        self.reserve_source = Some(reserva.source);
        self
    }

    /// Se alguma das duas fontes desenha `c`.
    pub fn cobre(&self, c: char) -> bool {
        self.face.glyph_id(c).0 != 0
            || self
                .reserva
                .as_ref()
                .is_some_and(|fonte| fonte.glyph_id(c).0 != 0)
    }

    /// Se há uma reserva carregada.
    pub fn tem_reserva(&self) -> bool {
        self.reserva.is_some()
    }

    /// De quem é o glifo de `c`: `true` quando veio da reserva.
    fn escolhe(&self, c: char) -> (bool, GlyphId) {
        let id = self.face.glyph_id(c);
        if id.0 != 0 {
            return (false, id);
        }
        match &self.reserva {
            Some(fonte) => {
                let da_reserva = fonte.glyph_id(c);
                match da_reserva.0 {
                    0 => (false, id),
                    _ => (true, da_reserva),
                }
            }
            None => (false, id),
        }
    }

    /// Altura da parte de cima da linha, no tamanho pedido. É o que o `GetFontMetrics` chama de
    /// `ascent`.
    pub fn ascent(&self, size: f32) -> u32 {
        self.face.as_scaled(PxScale::from(size)).ascent().ceil() as u32
    }

    /// Altura da parte de baixo, em valor positivo — o `descent` do BREW é positivo, e o da
    /// fonte é negativo.
    pub fn descent(&self, size: f32) -> u32 {
        (-self.face.as_scaled(PxScale::from(size)).descent())
            .ceil()
            .max(0.0) as u32
    }

    /// Quanto o texto ocupa na horizontal, incluindo o espaçamento entre letras.
    ///
    /// Cada caractere é medido na fonte que o desenha: um ideograma vem da reserva com o avanço
    /// dela (um quadrado de um `em`), e não com o avanço do `.notdef` da fonte do jogo. Sem isso
    /// o texto chinês mede quase nada e a centralização de menu joga a linha para fora da tela.
    pub fn width(&self, text: &str, size: f32) -> u32 {
        let principal = self.face.as_scaled(PxScale::from(size));
        let reserva = self
            .reserva
            .as_ref()
            .map(|fonte| fonte.as_scaled(PxScale::from(size * self.escala_reserva)));
        let mut largura = 0.0;
        let mut anterior: Option<(bool, GlyphId)> = None;
        for c in text.chars() {
            let (da_reserva, id) = self.escolhe(c);
            let (kern, avanco) = match (da_reserva, &reserva) {
                (true, Some(fonte)) => (
                    anterior
                        .filter(|(antes, _)| *antes)
                        .map_or(0.0, |(_, antes)| fonte.kern(antes, id)),
                    fonte.h_advance(id),
                ),
                _ => (
                    anterior
                        .filter(|(antes, _)| !*antes)
                        .map_or(0.0, |(_, antes)| principal.kern(antes, id)),
                    principal.h_advance(id),
                ),
            };
            largura += kern + avanco;
            anterior = Some((da_reserva, id));
        }
        largura.ceil().max(0.0) as u32
    }

    /// Rasteriza `text` e devolve um glifo por caractere que tenha desenho.
    ///
    /// A origem é o **canto superior esquerdo** do texto, que é como o BREW posiciona — a
    /// linha de base entra aqui, somando o `ascent`, para quem chama não precisar saber disso.
    pub fn layout(&self, text: &str, size: f32) -> Vec<Glyph> {
        let escala = PxScale::from(size);
        let escala_reserva = PxScale::from(size * self.escala_reserva);
        let principal = self.face.as_scaled(escala);
        let reserva = self
            .reserva
            .as_ref()
            .map(|fonte| fonte.as_scaled(escala_reserva));
        // A linha de base é a da fonte do jogo: é ela que manda no espaçamento que o jogo já
        // reservou na tela. A reserva se alinha por essa mesma base.
        let base = principal.ascent();
        let mut saida = Vec::new();
        let mut caneta = 0.0;
        let mut anterior: Option<(bool, GlyphId)> = None;
        for c in text.chars() {
            let (da_reserva, id) = self.escolhe(c);
            let (kern, avanco) = match (da_reserva, &reserva) {
                (true, Some(fonte)) => (
                    anterior
                        .filter(|(antes, _)| *antes)
                        .map_or(0.0, |(_, antes)| fonte.kern(antes, id)),
                    fonte.h_advance(id),
                ),
                _ => (
                    anterior
                        .filter(|(antes, _)| !*antes)
                        .map_or(0.0, |(_, antes)| principal.kern(antes, id)),
                    principal.h_advance(id),
                ),
            };
            caneta += kern;
            let tamanho = match da_reserva {
                true => escala_reserva,
                false => escala,
            };
            let glifo = id.with_scale_and_position(tamanho, ab_glyph::point(caneta, base));
            caneta += avanco;
            anterior = Some((da_reserva, id));
            // Sem glifo em nenhuma das duas: avança a caneta e não desenha. Desenhar o
            // `.notdef` encheria a linha de caixinhas, que é pior do que deixar o espaço.
            if id.0 == 0 {
                continue;
            }
            let desenho = match (da_reserva, &reserva) {
                (true, Some(fonte)) => fonte.outline_glyph(glifo),
                _ => principal.outline_glyph(glifo),
            };
            // Espaço não tem contorno: avança a caneta e não desenha nada.
            let Some(desenho) = desenho else {
                continue;
            };
            let caixa = desenho.px_bounds();
            let (largura, altura) = (caixa.width() as u32, caixa.height() as u32);
            if largura == 0 || altura == 0 {
                continue;
            }
            let mut coverage = vec![0u8; (largura * altura) as usize];
            desenho.draw(|x, y, v| {
                if x < largura && y < altura {
                    coverage[(y * largura + x) as usize] =
                        (v * 255.0).round().clamp(0.0, 255.0) as u8;
                }
            });
            saida.push(Glyph {
                x: caixa.min.x as i32,
                y: caixa.min.y as i32,
                width: largura,
                height: altura,
                coverage,
            });
        }
        saida
    }
}

/// Quanto o `PxScale` da reserva tem de crescer para ela ter o mesmo `em` da principal.
///
/// O `PxScale` do `ab_glyph` não é "pixels por `em`": ele é normalizado pela altura da fonte
/// (`ascent - descent`), que muda de fonte para fonte. A `Noto Sans CJK` tem altura de 1,45 `em`
/// e a `tectoy.ttf`, 1,16 — com o mesmo `PxScale`, o ideograma saía 20% menor que a letra latina
/// ao lado, num tamanho em que cada pixel de altura conta.
fn escala_para_em(principal: &FontVec, reserva: &FontVec) -> f32 {
    let razao = |fonte: &FontVec| match fonte.units_per_em() {
        Some(upem) if upem > 0.0 => fonte.height_unscaled() / upem,
        _ => 1.0,
    };
    let (da_principal, da_reserva) = (razao(principal), razao(reserva));
    if da_principal <= 0.0 || da_reserva <= 0.0 {
        return 1.0;
    }
    // Um fator absurdo significa métrica estranha: melhor não mexer no tamanho.
    (da_reserva / da_principal).clamp(0.5, 2.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fonte que a Z-Wheel empacota. O teste só roda quando ela está no cache — é material do
    /// usuário, não do repositório.
    fn tectoy() -> Option<Font> {
        let cache = dirs_cache()?;
        let ttf = std::fs::read(cache).ok()?;
        Font::load(ttf, "tectoy.ttf".into())
    }

    fn dirs_cache() -> Option<std::path::PathBuf> {
        let base = std::env::var_os("HOME")?;
        let raiz = std::path::Path::new(&base).join(".config/zeebx/cache");
        std::fs::read_dir(raiz)
            .ok()?
            .filter_map(Result::ok)
            .map(|e| e.path())
            .find(|p| {
                p.file_name()
                    .is_some_and(|n| n.to_string_lossy().starts_with("Z-Wheel"))
            })
            .map(|p| p.join("mod/274755/tectoy.ttf"))
            .filter(|p| p.exists())
    }

    /// A fonte de reserva que esta máquina tem. Em CI costuma não haver nenhuma, e aí o teste de
    /// reserva se declara pulado em vez de falhar.
    pub(super) fn reserva() -> Option<Font> {
        crate::video::font_sistema::abre(std::path::Path::new("/nao-existe"))
    }

    /// Uma fonte **sem** ideograma nenhum, para exercitar a reserva: a da Z-Wheel quando existe,
    /// e a DejaVu do sistema quando não.
    pub(super) fn fonte_sem_cjk() -> Option<Font> {
        if let Some(fonte) = tectoy() {
            return Some(fonte);
        }
        let bytes = std::fs::read("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf").ok()?;
        Font::load(bytes, "DejaVuSans.ttf".into())
    }

    #[test]
    fn o_que_nao_e_fonte_e_recusado() {
        assert!(Font::load(b"nao sou uma fonte".to_vec(), "x".into()).is_none());
        assert!(Font::load(Vec::new(), "x".into()).is_none());
    }

    #[test]
    fn a_fonte_do_pacote_mede_e_desenha() {
        let Some(fonte) = tectoy() else {
            // Sem o jogo no cache não há o que testar: o teste existe para quem tem o material.
            return;
        };
        // Uma linha de 16 pixels tem altura de linha plausível, não zero e não absurda.
        let (ascent, descent) = (fonte.ascent(16.0), fonte.descent(16.0));
        assert!(
            (8..40).contains(&ascent),
            "ascent fora do razoável: {ascent}"
        );
        assert!(descent < 20, "descent fora do razoável: {descent}");

        // Texto mais longo ocupa mais espaço, e o espaço em branco conta.
        assert!(fonte.width("Jogar", 16.0) > fonte.width("J", 16.0));
        assert!(fonte.width("a a", 16.0) > fonte.width("aa", 16.0));

        // "Jogar" tem cinco letras e todas desenham alguma coisa.
        let glifos = fonte.layout("Jogar", 16.0);
        assert_eq!(glifos.len(), 5);
        assert!(glifos.iter().all(|g| g.coverage.iter().any(|&v| v > 0)));

        // O espaço não vira glifo, mas empurra o que vem depois.
        let com_espaco = fonte.layout("a b", 16.0);
        assert_eq!(com_espaco.len(), 2);
        assert!(com_espaco[1].x > com_espaco[0].x);
    }

    #[test]
    fn a_reserva_desenha_o_que_a_principal_nao_tem() {
        let (Some(principal), Some(reserva)) = (fonte_sem_cjk(), reserva()) else {
            return;
        };
        // As fontes latinas não têm ideograma nenhum: é essa a premissa do teste.
        assert!(
            !principal.cobre('中'),
            "a fonte de teste cobriu CJK; revisar"
        );
        assert!(reserva.cobre('中'));

        let fonte = principal.com_reserva(reserva);
        assert!(fonte.tem_reserva());
        assert!(fonte.cobre('中'));

        // Dois ideogramas desenham os dois glifos, não nenhum.
        let glifos = fonte.layout("中文", 16.0);
        assert_eq!(glifos.len(), 2);
        assert!(glifos.iter().all(|g| g.coverage.iter().any(|&v| v > 0)));

        // Cada ideograma ocupa um quadrado de um `em`, e o `em` da reserva foi ajustado para o
        // da fonte do jogo: a 16 px, um ideograma não pode sair com 9 px de lado.
        let (um, dois) = (fonte.width("中", 16.0), fonte.width("中文", 16.0));
        assert!(um >= 12, "ideograma pequeno demais: {um} px");
        assert!(
            (dois as i64 - 2 * um as i64).abs() <= 2,
            "dois ideogramas medem {dois} e um mede {um}"
        );
        // A altura da caixa também confere com a de uma letra latina da mesma linha.
        let ideograma = &fonte.layout("中", 16.0)[0];
        assert!(
            ideograma.height >= 12 && ideograma.height <= 16,
            "altura do ideograma fora do razoável: {}",
            ideograma.height
        );
        // A largura cresce junto com o número de ideogramas.
        assert!(fonte.width("中文字", 16.0) > dois);
    }

    #[test]
    fn o_latim_continua_saindo_da_fonte_do_jogo() {
        let (Some(principal), Some(reserva)) = (fonte_sem_cjk(), reserva()) else {
            return;
        };
        let sozinha = principal.width("Jogar", 16.0);
        let com_reserva = principal.com_reserva(reserva).width("Jogar", 16.0);
        // A reserva não encosta no que a principal já desenha.
        assert_eq!(sozinha, com_reserva);
    }
}

#[cfg(test)]
mod prova_visual {
    use super::tests;
    use super::*;

    /// Despeja um PGM do texto desenhado como o `display.rs` desenha, para olhar o resultado.
    /// Roda só a pedido: `cargo test -- --ignored prova_visual`.
    ///
    /// Sai um par de imagens: a do jogo sozinha (o que se via antes da reserva) e a com a reserva.
    #[test]
    #[ignore]
    fn desenha_um_paragrafo_em_chines() {
        let (Some(base), Some(outra), Some(reserva)) =
            (tests::fonte_sem_cjk(), tests::fonte_sem_cjk(), tests::reserva())
        else {
            return;
        };
        let com_reserva = outra.com_reserva(reserva);
        let linhas = [
            "中文测试：你好，世界",
            "按键开始游戏 A B C 123",
            "设置 存档 退出 继续",
        ];
        despeja("/tmp/cjk_antes.pgm", &base, &linhas);
        despeja("/tmp/cjk_depois.pgm", &com_reserva, &linhas);
        println!("escrito /tmp/cjk_antes.pgm e /tmp/cjk_depois.pgm");
    }

    fn despeja(caminho: &str, fonte: &Font, linhas: &[&str]) {
        let (largura, altura) = (320usize, 66usize);
        let mut tela = vec![255u8; largura * altura];
        for (i, texto) in linhas.iter().enumerate() {
            let y = 4 + i as i32 * 20;
            for g in fonte.layout(texto, 16.0) {
                for ly in 0..g.height as i32 {
                    for lx in 0..g.width as i32 {
                        let cobertura = g.coverage[(ly as u32 * g.width + lx as u32) as usize];
                        if cobertura < 128 {
                            continue;
                        }
                        let (px, py) = (g.x + lx + 4, y + g.y + ly);
                        if (0..largura as i32).contains(&px) && (0..altura as i32).contains(&py) {
                            tela[py as usize * largura + px as usize] = 0;
                        }
                    }
                }
            }
        }
        let mut saida = format!("P5\n{largura} {altura}\n255\n").into_bytes();
        saida.extend_from_slice(&tela);
        std::fs::write(caminho, saida).unwrap();
    }
}
