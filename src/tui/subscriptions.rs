use crate::cli::StreamMode;
use std::collections::HashMap;

pub const TOKEN_BUDGET: usize = 3000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Reserve {
    Target,
    Visible,
    Wing,
}

impl Reserve {
    const ALL: [Reserve; 3] = [Reserve::Target, Reserve::Visible, Reserve::Wing];
}

#[derive(Debug, Default, Clone)]
pub struct Demands {
    claims: Vec<(Reserve, u32, StreamMode)>,
}

impl Demands {
    pub fn want(&mut self, reserve: Reserve, token: u32, mode: StreamMode) {
        if token == 0 {
            return;
        }
        self.claims.push((reserve, token, mode));
    }

    pub fn absorb(&mut self, other: Demands) {
        self.claims.extend(other.claims);
    }

    pub fn resolve(&self, budget: usize) -> HashMap<u32, StreamMode> {
        let mut desired: HashMap<u32, StreamMode> = HashMap::new();
        for tier in Reserve::ALL {
            for (reserve, token, mode) in &self.claims {
                if *reserve != tier {
                    continue;
                }
                match desired.get_mut(token) {
                    Some(current) => {
                        if mode_rank(*mode) > mode_rank(*current) {
                            *current = *mode;
                        }
                    }
                    None => {
                        if desired.len() < budget {
                            desired.insert(*token, *mode);
                        }
                    }
                }
            }
        }
        desired
    }
}

fn mode_rank(mode: StreamMode) -> u8 {
    match mode {
        StreamMode::Ltp => 0,
        StreamMode::Quote => 1,
        StreamMode::Full => 2,
    }
}

#[cfg(test)]
mod tests {
    use super::{Demands, Reserve};
    use crate::cli::StreamMode;

    #[test]
    fn duplicate_claims_collapse_to_the_strongest_mode() {
        let mut demands = Demands::default();
        demands.want(Reserve::Visible, 1, StreamMode::Quote);
        demands.want(Reserve::Target, 1, StreamMode::Full);
        demands.want(Reserve::Visible, 2, StreamMode::Ltp);
        demands.want(Reserve::Visible, 2, StreamMode::Quote);

        let desired = demands.resolve(100);
        assert_eq!(desired.len(), 2);
        assert_eq!(desired[&1], StreamMode::Full);
        assert_eq!(desired[&2], StreamMode::Quote);
    }

    #[test]
    fn token_zero_is_never_claimed() {
        let mut demands = Demands::default();
        demands.want(Reserve::Target, 0, StreamMode::Full);
        assert!(demands.resolve(100).is_empty());
    }

    #[test]
    fn the_budget_is_filled_in_reservation_order() {
        let mut demands = Demands::default();
        demands.want(Reserve::Wing, 30, StreamMode::Full);
        demands.want(Reserve::Visible, 20, StreamMode::Quote);
        demands.want(Reserve::Target, 10, StreamMode::Full);

        let desired = demands.resolve(2);
        assert_eq!(desired.len(), 2);
        assert_eq!(desired[&10], StreamMode::Full, "the target is reserved");
        assert_eq!(desired[&20], StreamMode::Quote, "then the visible row");
        assert!(!desired.contains_key(&30), "the wing dropped");
    }

    #[test]
    fn a_full_budget_still_raises_an_admitted_token() {
        let mut demands = Demands::default();
        demands.want(Reserve::Visible, 10, StreamMode::Quote);
        demands.want(Reserve::Wing, 10, StreamMode::Full);
        demands.want(Reserve::Wing, 11, StreamMode::Full);

        let desired = demands.resolve(1);
        assert_eq!(desired.len(), 1);
        assert_eq!(desired[&10], StreamMode::Full);
    }

    #[test]
    fn absorbing_keeps_every_consumers_claim() {
        let mut chart = Demands::default();
        chart.want(Reserve::Target, 10, StreamMode::Full);
        let mut watch = Demands::default();
        watch.want(Reserve::Visible, 10, StreamMode::Quote);
        watch.want(Reserve::Visible, 11, StreamMode::Quote);

        let mut union = Demands::default();
        union.absorb(chart);
        union.absorb(watch.clone());
        assert_eq!(union.resolve(100)[&10], StreamMode::Full);

        let mut without_chart = Demands::default();
        without_chart.absorb(watch);
        let desired = without_chart.resolve(100);
        assert_eq!(desired[&10], StreamMode::Quote);
        assert_eq!(desired.len(), 2);
    }
}
