use twitchchat::messages::Privmsg;

pub struct Command<'a> {
    pub cmd: &'a str,
    pub args: Option<&'a str>,
    pub msg: &'a Privmsg<'a>,
    pub channel: &'a str,
}

impl<'a> Command<'a> {
    pub fn parse(msg: &'a Privmsg<'a>) -> Option<Command<'a>> {
        const TRIGGER: &str = "!";

        let data = msg.data();
        if !data.starts_with(TRIGGER) || data.len() == TRIGGER.len() {
            return None;
        }

        let mut iter = data.splitn(2, ' ');
        let (head, tail) = (iter.next()?, iter.next());

        Some(Command {
            cmd: &head[TRIGGER.len()..],
            args: tail,
            msg,
            channel: &msg.channel()[1..],
        })
    }
}
