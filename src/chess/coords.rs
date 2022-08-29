use std::fmt::Debug;

///Utility type to hold a set of [`u8`] coordinates in an `(x, y)` format. Can also represent a piece which was taken.
///
/// (0, 0) is at the top left, with y counting the rows, and x counting the columns
#[derive(Copy, Clone, PartialEq, Eq, Default)]
pub enum Coords {
    ///The coordinate is currently off the board, or a taken piece
    #[default]
    OffBoard,
    ///The coordinate is currently on the board at these coordinates.
    OnBoard(u8, u8), //could use one u8 but cba
}

impl Debug for Coords {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Coords::OffBoard => f.debug_struct("Coords").finish(),
            Coords::OnBoard(x, y) => f
                .debug_struct("Coords")
                .field("x", x)
                .field("y", y)
                .finish(),
        }
    }
}

impl TryFrom<(i32, i32)> for Coords {
    type Error = anyhow::Error;

    fn try_from((x, y): (i32, i32)) -> Result<Self, Self::Error> {
        if x == -1 && y == -1 {
            return Ok(Self::OffBoard);
        }

        if x < 0 {
            bail!("x < 0")
        }
        if x > 7 {
            bail!("x > 7")
        }
        if y < 0 {
            bail!("y < 0")
        }
        if y > 7 {
            bail!("y > 7")
        }

        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        Ok(Self::OnBoard(x as u8, y as u8)) //conversion works as all checked above
    }
}
impl TryFrom<(u32, u32)> for Coords {
    type Error = anyhow::Error;

    fn try_from((x, y): (u32, u32)) -> Result<Self, Self::Error> {
        if x > 7 {
            bail!("x > 7")
        }
        if y > 7 {
            bail!("y > 7")
        }

        #[allow(clippy::cast_possible_truncation)]
        Ok(Self::OnBoard(x as u8, y as u8)) //conversion works as all checked above
    }
}

impl From<Coords> for Option<(u8, u8)> {
    fn from(c: Coords) -> Self {
        c.to_option()
    }
}
impl From<(u8, u8)> for Coords {
    fn from((x, y): (u8, u8)) -> Self {
        Self::OnBoard(x, y)
    }
}

impl Coords {
    ///Provides an index with which to index a 1D array using the 2D coords, assuming there are 8 rows per column
    #[must_use]
    pub fn to_usize(&self) -> Option<usize> {
        match self {
            Coords::OffBoard => None,
            Coords::OnBoard(x, y) => Some((y * 8 + x) as usize),
        }
    }
    ///Provides the X part of the coordinate
    #[must_use]
    pub fn x(&self) -> Option<u8> {
        self.to_option().map(|(x, _)| x)
    }
    ///Provides the Y part of the coordinate
    #[must_use]
    pub fn y(&self) -> Option<u8> {
        self.to_option().map(|(_, y)| y)
    }

    ///Provides a utility function for turning `Coords` to an `Option<(u8, u8)>`
    #[must_use]
    pub fn to_option(&self) -> Option<(u8, u8)> {
        match *self {
            Coords::OffBoard => None,
            Coords::OnBoard(x, y) => Some((x, y)),
        }
    }

    ///Utility function for whether or not it is taken
    #[must_use]
    pub fn is_taken(&self) -> bool {
        matches!(self, Coords::OffBoard)
    }

    ///Utility function for whether or not it is on the board
    #[must_use]
    pub fn is_on_board(&self) -> bool {
        matches!(self, Coords::OnBoard(_, _))
    }
}
