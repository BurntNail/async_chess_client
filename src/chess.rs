use strum::{EnumIter, IntoEnumIterator, Display};

#[derive(EnumIter, Display, Copy, Clone)]
pub enum ChessPieceKind {
	Bishop,
	Knight,
	Pawn,
	Queen,
	King,
	Rook
}

pub struct ChessPiece {
	pub kind: ChessPieceKind,
	pub is_white: bool
}
impl ChessPiece {
	pub fn all_variants() -> Vec<Self> {
		let mut v = Vec::with_capacity(12);

		for el in ChessPieceKind::iter() {
			v.push(Self {
				kind: el,
				is_white: false
			});
			v.push(Self {
				kind: el,
				is_white: true
			});
		}

		v
	}

	pub fn to_file_name (&self) -> String {
		format!("{}_{}.png", if self.is_white { "white" } else { "black" }, self.kind.to_string().to_lowercase())
	}
}