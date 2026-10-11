pub mod client;
pub mod harness;

#[cfg(test)]
mod test_create_auction;
#[cfg(test)]
mod test_declare_winner;
#[cfg(test)]
mod test_escape;
#[cfg(test)]
mod test_events;
#[cfg(test)]
mod test_forfeit;
#[cfg(test)]
mod test_pay_remainder;
#[cfg(test)]
mod test_place_deposit;
#[cfg(test)]
mod test_refund_deposit;
#[cfg(test)]
mod test_scenarios;
#[cfg(test)]
mod test_settle;
