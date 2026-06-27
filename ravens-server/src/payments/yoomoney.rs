//! YooMoney (ЮMoney) payment link generator.
//! Uses the Quickpay redirect — no server-to-server call needed to create the link.

pub fn create_payment_url(receiver: &str, label: &str, amount: f64, comment: &str) -> String {
    format!(
        "https://yoomoney.ru/quickpay/confirm?receiver={receiver}&quickpay-form=button\
         &paymentType=AC&sum={amount:.2}&label={label}&comment={comment}&targets=RavensNexus+{comment}+subscription",
        receiver = urlencoding(receiver),
        amount   = amount,
        label    = urlencoding(label),
        comment  = urlencoding(comment),
    )
}

fn urlencoding(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            _ => { out.push('%'); out.push_str(&format!("{:02X}", b)); }
        }
    }
    out
}
