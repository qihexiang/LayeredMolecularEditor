use serde::{Deserialize, Serialize};
use std::collections::btree_set::IntoIter;
use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;

type NtoNData = BTreeSet<(String, usize)>;

#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(from = "FriendlyNtoN")]
pub struct NtoN(NtoNData);

impl NtoN {
    pub fn new() -> Self {
        Self(BTreeSet::new())
    }

    pub fn data(&self) -> &NtoNData {
        &self.0
    }

    fn data_mut(&mut self) -> &mut NtoNData {
        &mut self.0
    }

    pub fn get_lefts(&self) -> BTreeSet<&String> {
        self.data().iter().map(|(l, _)| l).collect()
    }

    pub fn get_rights(&self) -> BTreeSet<&usize> {
        self.data().iter().map(|(_, r)| r).collect()
    }

    pub fn get_left<'a>(&'a self, left: &'a String) -> impl Iterator<Item = &usize> {
        self.data()
            .iter()
            .filter_map(move |(l, r)| if l == left { Some(r) } else { None })
    }

    pub fn get_right<'a>(&'a self, right: &'a usize) -> impl Iterator<Item = &String> {
        self.data()
            .iter()
            .filter_map(move |(l, r)| if r == right { Some(l) } else { None })
    }

    pub fn insert(&mut self, left: String, right: usize) -> bool {
        self.data_mut().insert((left, right))
    }

    pub fn insert_left<T>(&mut self, left: String, rights: T)
    where
        T: Iterator<Item = usize>,
    {
        let rights = rights.into_iter().map(|right| (left.clone(), right));
        self.data_mut().extend(rights);
    }

    pub fn insert_right<T>(&mut self, right: usize, lefts: T)
    where
        T: Iterator<Item = String>,
    {
        let lefts = lefts.into_iter().map(|left| (left, right.clone()));
        self.data_mut().extend(lefts);
    }

    pub fn remove(&mut self, left: &String, right: &usize) -> bool {
        self.data_mut().remove(&(left.clone(), right.clone()))
    }

    pub fn remove_left(&mut self, left: &String) {
        self.data_mut().retain(|(l, _)| l != left)
    }

    pub fn remove_right(&mut self, right: &usize) {
        self.data_mut().retain(|(_, r)| r != right)
    }

    pub fn extend<I>(&mut self, iter: I)
    where
        I: IntoIterator<Item = (String, usize)>,
    {
        self.data_mut().extend(iter)
    }

    pub fn overlay_to(&self, other: &Self) -> Self {
        let mut overlayed = other.clone();
        overlayed.extend(self.data().clone());
        overlayed
    }
}

impl<T: Iterator<Item = (String, usize)>> From<T> for NtoN {
    fn from(value: T) -> Self {
        Self(value.collect())
    }
}

impl Into<NtoNData> for NtoN {
    fn into(self) -> NtoNData {
        self.0
    }
}

impl IntoIterator for NtoN {
    type Item = (String, usize);
    type IntoIter = IntoIter<(String, usize)>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl FromIterator<(String, usize)> for NtoN {
    fn from_iter<T: IntoIterator<Item = (String, usize)>>(iter: T) -> Self {
        Self::from(iter.into_iter())
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
enum IndexCollect {
    Collect(BTreeSet<usize>),
    Range(Range<usize>),
}

impl IndexCollect {
    fn collect(self) -> BTreeSet<usize> {
        match self {
            IndexCollect::Collect(value) => value,
            IndexCollect::Range(range) => range.collect(),
        }
    }
}

#[derive(Deserialize)]
pub struct FriendlyNtoN(BTreeMap<String, IndexCollect>);

impl From<FriendlyNtoN> for NtoN {
    fn from(value: FriendlyNtoN) -> Self {
        Self::from_iter(
            value
                .0
                .into_iter()
                .map(|(k, v)| {
                    v.collect().into_iter().map(move |v| ((&k).to_string(), v))
                })
                .flatten()
        )
    }
}
