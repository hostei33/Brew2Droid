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
//! # As reservas
//!
//! A fonte do jogo desenha o que o jogo escreve no idioma para o qual foi feito. Uma tradução
//! troca as frases e não troca o `.ttf`: o chinês de uma tradução não tem glifo nenhum na
//! `tectoy.ttf`, e cada ideograma saía como nada — a frase ficava só com os números.
//!
//! Por isso a fonte do jogo pode vir acompanhada de **reservas**, consultadas apenas caractere a
//! caractere, e só quando a principal devolve `.notdef`. Elas não substituem a fonte do jogo (o
//! latim continua saindo com as formas dele, e as métricas da linha também): só preenchem o que
//! faltaria. Quem escolhe as reservas é o [`super::font_sistema`].
//!
//! São várias porque nenhuma cobre todos os ideogramas, e porque a do jogo pode cobrir menos
//! ainda: `NotoSansSC` tem 30 mil glifos, `DroidSansFallback` bem menos, e o jogo pode trazer uma
//! fonte que só tem latim. Elas são lidas **sob demanda**: guardar a `Noto Sans CJK` inteira na
//! memória para desenhar um menu em português custaria 20 MB, e a maioria dos jogos nunca vai
//! precisar dela.
//!
//! # O que este motor não faz
//!
//! Ele desenha **um caractere por vez**. Isso basta para chinês, japonês, coreano, latim, grego,
//! cirílico e vietnamita, que não têm composição. Não basta para árabe (letras que se ligam e texto
//! da direita para a esquerda), tailandês (vogal empilhada sobre a consoante) nem devanágari
//! (consoantes conjuntas) — ali o glifo aparece e a palavra fica errada. Fazer isso direito pede um
//! motor de *shaping* (HarfBuzz) e uma passada de bidi, que este port não tem; por isso o
//! [`super::font_sistema`] só procura fontes CJK.

use std::cell::RefCell;
use std::path::PathBuf;

use ab_glyph::{Font as _, FontVec, GlyphId, PxScale, ScaleFont as _};

/// Uma fonte que só entra quando a do jogo não desenha o caractere.
struct Reserva {
    face: FontVec,
    /// O ajuste de tamanho, para ela ter o mesmo `em` da principal.
    escala: f32,
    source: String,
}

/// Uma fonte do sistema que ainda não foi lida.
///
/// Ler é caro — a `Noto Sans CJK` tem 20 MB — e a maioria dos jogos nunca precisa dela. Guardar o
/// caminho e só abrir quando aparece um caractere que ninguém aqui desenha evita carregar o
/// alfabeto errado.
pub struct Candidata {
    caminho: PathBuf,
    /// As faixas de código que ela atende.
    ///
    /// Vazia significa "qualquer escrita": é o caso do que o usuário põe na pasta de fontes ou
    /// aponta em `ZEEBX_CJK_FONT`, e dela se exige apenas que desenhe algum dos caracteres que
    /// faltam. Com faixas, a fonte precisa desenhar **todos** os caracteres de validação — senão um
    /// `.ttf` de nome parecido toma o lugar de outro que serviria.
    faixas: &'static [(u32, u32)],
}

/// O que uma fonte tem de saber desenhar para ser aceita, e o texto usado como prova.
///
/// Dois ideogramas, e não um: um arquivo que só tem `中` (um recorte de subconjunto) desenharia a
/// metade das frases. Dois é o mínimo que separa "fonte de chinês" de "fonte com um ideograma".
const SONDA: [char; 2] = ['中', '文'];

impl Candidata {
    pub fn nova(caminho: PathBuf, faixas: &'static [(u32, u32)]) -> Self {
        Self { caminho, faixas }
    }

    /// Lê a fonte exigindo que ela cubra a sonda.
    ///
    /// É o caminho de quem vira **principal** (o jogo não trouxe `.ttf`): aí não há "caracteres
    /// que faltam" para servir de critério, e o critério é desenhar chinês.
    pub fn abre(&self) -> Option<Font> {
        self.carrega(&SONDA)
    }

    /// Se vale a pena abrir esta fonte para os caracteres que faltam.
    fn interessa(&self, faltando: &[char]) -> bool {
        if self.faixas.is_empty() {
            return !faltando.is_empty();
        }
        faltando.iter().any(|c| self.atende(*c))
    }

    /// Se `c` cai numa das faixas desta escrita.
    fn atende(&self, c: char) -> bool {
        let codigo = c as u32;
        self.faixas
            .iter()
            .any(|(inicio, fim)| codigo >= *inicio && codigo <= *fim)
    }

    /// Lê, escolhe a face e valida. `None` quando o arquivo não abre ou não desenha o que prometia.
    fn carrega(&self, faltando: &[char]) -> Option<Font> {
        let exigidos: Vec<char> = match self.faixas.is_empty() {
            true => faltando.to_vec(),
            false => SONDA.to_vec(),
        };
        let bytes = std::fs::read(&self.caminho).ok()?;
        let indice = melhor_face(&bytes, &exigidos);
        let fonte = Font::load_index(bytes, indice, nome_do(&self.caminho))?;
        let desenha = |c: char| fonte.face.glyph_id(c).0 != 0;
        let aceita = match self.faixas.is_empty() {
            // Sem faixas basta cobrir parte do que falta: um arquivo pode ter só alguns glifos.
            true => exigidos.iter().any(|c| desenha(*c)),
            // Com faixas tem de cobrir a sonda inteira, senão um `.ttf` de nome parecido toma o
            // lugar de outro que serviria.
            false => exigidos.iter().all(|c| desenha(*c)),
        };
        if !aceita {
            return None;
        }
        crate::registro!(
            crate::registro::Nivel::Informacao,
            "texto",
            "fonte de reserva: {} (face {indice})",
            self.caminho.display()
        );
        Some(fonte)
    }
}

/// Uma fonte carregada de um `.ttf` do próprio jogo.
pub struct Font {
    face: FontVec,
    /// As fontes do sistema, na ordem em que foram aceitas.
    reservas: RefCell<Vec<Reserva>>,
    /// As que ainda não foram lidas.
    candidatas: RefCell<Vec<Candidata>>,
    /// De onde ela veio, para o relatório.
    pub source: String,
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
                reservas: RefCell::new(Vec::new()),
                candidatas: RefCell::new(Vec::new()),
                source,
            })
    }

    /// Põe `reserva` atrás desta fonte, para os caracteres que ela não desenha.
    pub fn com_reserva(self, reserva: Font) -> Self {
        self.com_reservas(vec![reserva])
    }

    /// Põe várias fontes atrás desta, na ordem dada.
    ///
    /// A ordem importa: a primeira que tiver o glifo ganha. Quem monta a lista é o
    /// [`super::font_sistema`], e ele põe o que o usuário escolheu antes das do sistema.
    pub fn com_reservas(self, reservas: Vec<Font>) -> Self {
        {
            let mut minhas = self.reservas.borrow_mut();
            for outra in reservas {
                minhas.push(Reserva {
                    escala: escala_para_em(&self.face, &outra.face),
                    face: outra.face,
                    source: outra.source,
                });
            }
        }
        self
    }

    /// As fontes que ainda podem ser lidas quando faltar um caractere.
    pub fn com_candidatas(self, candidatas: Vec<Candidata>) -> Self {
        *self.candidatas.borrow_mut() = candidatas;
        self
    }

    /// Se alguma das fontes já carregadas desenha `c`.
    ///
    /// Não lê candidatas: é a pergunta "isto está resolvido agora?", e quem responde por elas é o
    /// [`Font::prepara`].
    pub fn cobre(&self, c: char) -> bool {
        if self.face.glyph_id(c).0 != 0 {
            return true;
        }
        self.reservas
            .borrow()
            .iter()
            .any(|r| r.face.glyph_id(c).0 != 0)
    }

    /// Se há alguma reserva carregada.
    pub fn tem_reserva(&self) -> bool {
        !self.reservas.borrow().is_empty()
    }

    /// De onde vieram as reservas, para o relatório.
    pub fn fontes_de_reserva(&self) -> Vec<String> {
        self.reservas
            .borrow()
            .iter()
            .map(|r| r.source.clone())
            .collect()
    }

    /// Lê a reserva de que `text` precisa, se ainda houver alguma candidata para isso.
    ///
    /// Roda antes de medir e de desenhar porque um caractere sem fonte não pode ser resolvido no
    /// meio do laço: a primeira linha sairia pela metade e a segunda inteira.
    fn prepara(&self, text: &str) {
        let faltando: Vec<char> = {
            let reservas = self.reservas.borrow();
            text.chars()
                .filter(|c| !c.is_control())
                .filter(|c| {
                    self.face.glyph_id(*c).0 == 0
                        && !reservas.iter().any(|r| r.face.glyph_id(*c).0 != 0)
                })
                .collect()
        };
        if faltando.is_empty() {
            return;
        }

        let mut pendentes = std::mem::take(&mut *self.candidatas.borrow_mut());
        let mut restantes = Vec::new();
        let mut novas = Vec::new();
        for candidata in pendentes.drain(..) {
            if !candidata.interessa(&faltando) {
                restantes.push(candidata);
                continue;
            }
            // Uma que não abre, ou que não desenha o que prometia, é descartada: tentar de novo a
            // cada quadro só custaria uma leitura de arquivo por quadro.
            if let Some(fonte) = candidata.carrega(&faltando) {
                novas.push(fonte);
            }
        }
        *self.candidatas.borrow_mut() = restantes;

        let mut reservas = self.reservas.borrow_mut();
        for outra in novas {
            reservas.push(Reserva {
                escala: escala_para_em(&self.face, &outra.face),
                face: outra.face,
                source: outra.source,
            });
        }
    }

    /// De quem é o glifo de `c`: `0` para a fonte do jogo, `n` para a reserva `n - 1`.
    fn escolhe(&self, c: char) -> (usize, GlyphId) {
        let id = self.face.glyph_id(c);
        if id.0 != 0 {
            return (0, id);
        }
        for (indice, reserva) in self.reservas.borrow().iter().enumerate() {
            let outro = reserva.face.glyph_id(c);
            if outro.0 != 0 {
                return (indice + 1, outro);
            }
        }
        // Ninguém tem: devolve o `.notdef` da principal, e quem chama avança a caneta sem
        // desenhar.
        (0, id)
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
        self.prepara(text);
        let principal = self.face.as_scaled(PxScale::from(size));
        let reservas = self.reservas.borrow();
        let escaladas: Vec<_> = reservas
            .iter()
            .map(|r| r.face.as_scaled(PxScale::from(size * r.escala)))
            .collect();
        let mut largura = 0.0;
        let mut anterior: Option<(usize, GlyphId)> = None;
        for c in text.chars() {
            let (origem, id) = self.escolhe(c);
            // `kerning` só faz sentido entre dois glifos da mesma face: entre uma letra do jogo e
            // um ideograma da reserva não há par para consultar.
            let (kern, avanco) = match origem.checked_sub(1).and_then(|i| escaladas.get(i)) {
                Some(fonte) => (
                    anterior
                        .filter(|(antes, _)| *antes == origem)
                        .map_or(0.0, |(_, antes)| fonte.kern(antes, id)),
                    fonte.h_advance(id),
                ),
                None => (
                    anterior
                        .filter(|(antes, _)| *antes == 0)
                        .map_or(0.0, |(_, antes)| principal.kern(antes, id)),
                    principal.h_advance(id),
                ),
            };
            largura += kern + avanco;
            anterior = Some((origem, id));
        }
        largura.ceil().max(0.0) as u32
    }

    /// Rasteriza `text` e devolve um glifo por caractere que tenha desenho.
    ///
    /// A origem é o **canto superior esquerdo** do texto, que é como o BREW posiciona — a
    /// linha de base entra aqui, somando o `ascent`, para quem chama não precisar saber disso.
    pub fn layout(&self, text: &str, size: f32) -> Vec<Glyph> {
        self.prepara(text);
        let escala = PxScale::from(size);
        let principal = self.face.as_scaled(escala);
        let reservas = self.reservas.borrow();
        let escaladas: Vec<_> = reservas
            .iter()
            .map(|r| r.face.as_scaled(PxScale::from(size * r.escala)))
            .collect();
        // A linha de base é a da fonte do jogo: é ela que manda no espaçamento que o jogo já
        // reservou na tela. As reservas se alinham por essa mesma base.
        let base = principal.ascent();
        let mut saida = Vec::new();
        let mut caneta = 0.0;
        let mut anterior: Option<(usize, GlyphId)> = None;
        for c in text.chars() {
            let (origem, id) = self.escolhe(c);
            let (kern, avanco) = match origem.checked_sub(1).and_then(|i| escaladas.get(i)) {
                Some(fonte) => (
                    anterior
                        .filter(|(antes, _)| *antes == origem)
                        .map_or(0.0, |(_, antes)| fonte.kern(antes, id)),
                    fonte.h_advance(id),
                ),
                None => (
                    anterior
                        .filter(|(antes, _)| *antes == 0)
                        .map_or(0.0, |(_, antes)| principal.kern(antes, id)),
                    principal.h_advance(id),
                ),
            };
            caneta += kern;
            let tamanho = match origem.checked_sub(1).and_then(|i| reservas.get(i)) {
                Some(reserva) => PxScale::from(size * reserva.escala),
                None => escala,
            };
            let glifo = id.with_scale_and_position(tamanho, ab_glyph::point(caneta, base));
            caneta += avanco;
            anterior = Some((origem, id));
            // Sem glifo em nenhuma delas: avança a caneta e não desenha. Desenhar o `.notdef`
            // encheria a linha de caixinhas, que é pior do que deixar o espaço.
            if id.0 == 0 {
                continue;
            }
            let desenho = match origem.checked_sub(1).and_then(|i| escaladas.get(i)) {
                Some(fonte) => fonte.outline_glyph(glifo),
                None => principal.outline_glyph(glifo),
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

/// Qual face abrir dentro do arquivo, dado o que se quer desenhar.
///
/// Um `.ttf`/`.otf` tem uma só, e a resposta é zero. Uma coleção (`.ttc`) tem várias: a
/// `NotoSansCJK-Regular.ttc` põe JP, KR, SC, TC e HK no mesmo arquivo, e a face zero é a japonesa
/// — os ideogramas aparecem, mas com formas que num texto chinês se leem errado. A escolha é a
/// primeira que desenha o que se pediu, com preferência pela variante simplificada.
fn melhor_face(dados: &[u8], exigidos: &[char]) -> u32 {
    if dados.get(..4) != Some(b"ttcf") {
        return 0;
    }
    let Some(quantas) = ttf_parser::fonts_in_collection(dados) else {
        return 0;
    };
    let cjk = exigidos.iter().any(|c| e_ideograma(*c));
    let mut primeira = None;
    for indice in 0..quantas {
        let Ok(face) = ttf_parser::Face::parse(dados, indice) else {
            continue;
        };
        if !exigidos.iter().all(|c| face.glyph_index(*c).is_some()) {
            continue;
        }
        if cjk && e_simplificado(&face) {
            return indice;
        }
        primeira.get_or_insert(indice);
    }
    primeira.unwrap_or(0)
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

/// Os ideogramas, que são o que distingue a variante de uma coleção CJK.
fn e_ideograma(c: char) -> bool {
    matches!(c as u32, 0x3400..=0x9FFF | 0xF900..=0xFAFF | 0x20000..=0x2FA1F)
}

fn nome_do(caminho: &std::path::Path) -> String {
    caminho
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| caminho.display().to_string())
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

    /// A fonte de reserva CJK que esta máquina tem. Em CI costuma não haver nenhuma, e aí o teste
    /// se declara pulado em vez de falhar.
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

    /// Uma fonte do sistema pelo caminho, quando existe nesta máquina.
    fn da_maquina(caminho: &str) -> Option<Font> {
        Font::load(std::fs::read(caminho).ok()?, nome_do(caminho.as_ref()))
    }

    /// O arquivo de fonte CJK desta máquina, para os testes que precisam de uma candidata que
    /// ainda não foi lida.
    fn caminho_cjk() -> Option<std::path::PathBuf> {
        [
            "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/truetype/wqy/wqy-zenhei.ttc",
        ]
        .into_iter()
        .map(std::path::PathBuf::from)
        .find(|caminho| caminho.exists())
    }

    /// Uma fonte só de latim, sem ideograma nenhum: a Liberation não traz CJK, e é isso que os
    /// testes da reserva precisam.
    pub(super) fn fonte_latina() -> Option<Font> {
        da_maquina("/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf")
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

    /// Uma reserva que não desenha ideograma é pulada, e a seguinte assume.
    ///
    /// É o caso real: o usuário põe uma fonte na pasta e ela não é de chinês. Ela não pode
    /// bloquear as do sistema que vêm atrás.
    #[test]
    fn a_reserva_sem_o_glifo_e_pulada() {
        let (Some(principal), Some(cjk)) = (fonte_latina(), reserva()) else {
            return;
        };
        let Some(sem_ideograma) = da_maquina("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf")
        else {
            return;
        };
        assert!(!sem_ideograma.cobre('中'), "a DejaVu passou a ter CJK");
        let fonte = principal.com_reservas(vec![sem_ideograma, cjk]);
        assert!(fonte.cobre('中'));
        assert_eq!(fonte.layout("中文", 16.0).len(), 2);
    }

    /// A reserva só é lida quando faz falta. Sem isso, abrir um jogo em português carregaria a
    /// `Noto Sans CJK` inteira — 20 MB — para não desenhar um ideograma sequer.
    #[test]
    fn a_candidata_so_e_lida_quando_falta_caractere() {
        let (Some(principal), Some(caminho)) = (fonte_latina(), caminho_cjk()) else {
            return;
        };
        let fonte = principal.com_candidatas(vec![Candidata::nova(
            caminho,
            &[(0x4E00, 0x9FFF)],
        )]);
        // Texto que a principal dá conta: nada é lido.
        assert!(!fonte.layout("Jogar", 16.0).is_empty());
        assert!(!fonte.tem_reserva());

        // Texto que ela não tem: a candidata é lida e passa a desenhar.
        assert!(!fonte.cobre('中'));
        assert!(!fonte.layout("中文", 16.0).is_empty());
        assert!(fonte.tem_reserva());
        assert!(fonte.cobre('中'));
        // E uma vez lida, não é lida de novo.
        let quantas = fonte.fontes_de_reserva().len();
        let _ = fonte.layout("中文", 16.0);
        assert_eq!(fonte.fontes_de_reserva().len(), quantas);
    }

    /// Uma candidata que não desenha o que prometia é recusada — senão um `.ttf` de nome parecido
    /// ocuparia o lugar de outro que serviria, e o texto continuaria sem aparecer.
    #[test]
    fn a_candidata_que_nao_cumpre_a_sonda_e_recusada() {
        let Some(principal) = fonte_sem_cjk() else {
            return;
        };
        let caminho = "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf";
        if !std::path::Path::new(caminho).exists() {
            return;
        }
        // A sonda pede ideograma; a DejaVu não tem nenhum.
        let fonte =
            principal.com_candidatas(vec![Candidata::nova(caminho.into(), &[(0x4E00, 0x9FFF)])]);
        assert!(fonte.layout("中文", 16.0).is_empty());
        assert!(!fonte.tem_reserva());
    }
}

#[cfg(test)]
mod prova_visual {
    use super::tests;
    use super::*;

    /// Despeja um PGM do texto desenhado como o `display.rs` desenha, para olhar o resultado.
    /// Roda só a pedido: `cargo test -- --ignored prova_visual`.
    ///
    /// Sai um par de imagens: a do jogo sozinha (o que se via antes da reserva) e a com a
    /// reserva.
    #[test]
    #[ignore]
    fn desenha_um_paragrafo_em_chines() {
        let (Some(base), Some(outra), Some(reserva)) = (
            tests::fonte_latina(),
            tests::fonte_latina(),
            tests::reserva(),
        ) else {
            return;
        };
        let com_reserva = outra.com_reserva(reserva);
        let linhas = [
            "中文测试：你好，世界",
            "Press A B C 123",
            "这是一句比较长的中文，用来检查断行和字距。",
            "日本語のテスト",
            "한국어 테스트",
        ];
        despeja("/tmp/cjk_antes.pgm", &base, &linhas);
        despeja("/tmp/cjk_depois.pgm", &com_reserva, &linhas);
        println!("escrito /tmp/cjk_antes.pgm e /tmp/cjk_depois.pgm");
    }

    /// O que a `Noto Sans CJK` cobre além do chinês: kana, hangul, cirílico e grego saem certos
    /// com um glifo por caractere, sem *shaping*.
    #[test]
    #[ignore]
    fn desenha_o_que_a_noto_cjk_cobre() {
        let (Some(outra), Some(reserva)) = (tests::fonte_latina(), tests::reserva()) else {
            return;
        };
        let fonte = outra.com_reserva(reserva);
        let linhas = [
            "中文测试：你好，世界",
            "日本語のテスト",
            "한국어 테스트",
            "Привет, мир",
            "Καλημέρα κόσμε",
            "Tiếng Việt",
        ];
        despeja("/tmp/ok.pgm", &fonte, &linhas);
        println!("escrito /tmp/ok.pgm");
    }

    fn despeja(caminho: &str, fonte: &Font, linhas: &[&str]) {
        let (largura, altura) = (320usize, 126usize);
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
