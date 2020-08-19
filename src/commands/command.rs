use twitchchat::messages::Privmsg;

/// A parsed !command
pub struct Command<'a> {
    /// The command name
    pub cmd: &'a str,
    /// Optional args
    pub args: Option<&'a str>,
    /// Associated msg
    pub msg: &'a Privmsg<'a>,
    /// 'Normalized' channel (e.g. # removed)
    pub channel: &'a str,
}

impl<'a> Command<'a> {
    /// Attempts parse the command from this privmsg
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
