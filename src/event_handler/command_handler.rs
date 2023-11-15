use anyhow::{ anyhow, Result };
use serenity::model::prelude::application_command::ApplicationCommandInteraction;
use serenity::model::{
    prelude::{
        application_command::{ CommandDataOption, CommandDataOptionValue },
        command::CommandOptionType,
        Channel,
        PartialChannel,
        PartialMember,
        Role,
    },
    user::User,
};

/// If a command expects a number but an integer is given, do not worry
/// too much as this exists. This is here to take either and convert it into
/// the desired type.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub enum IntOrNumber {
    Int(i64),
    Number(f64),
}

impl IntOrNumber {
    /// Takes whatever option is in the enum
    /// and returns it as a f64.
    pub const fn cast_to_f64(&self) -> f64 {
        match self {
            Self::Int(i) => *i as f64, // Should not cause a lot of loss here.
            Self::Number(n) => *n,
        }
    }

    /// Takes whatever option is in the enum
    /// and returns it as an i64.
    pub const fn cast_to_i64(&self) -> i64 {
        match self {
            Self::Int(i) => *i,
            Self::Number(n) => *n as i64, // Ok now THIS causes loss, but I doubt this function will be used. Here just in case.
        }
    }
}

#[derive(Debug, Clone)]
pub struct CommandOptions {
    args: Vec<Opt>,
}

/// This exists because I do not need the unresolved value of [CommandDataOption], so this is
/// to contain everything else besides those. If I do happen to need it, I can get it from the
/// [ApplicationCommandInteraction] that is passed to a command.
#[derive(Debug, Clone)]
pub struct Opt {
    /// The name of the option as defined by the command.
    name: String,
    /// The kind of option which should match the one defined by the command.
    kind: CommandOptionType,
    /// If the option happens to be a subcommand or a subcommand group, this will contain the options of said
    /// subcommand or subcommand group.
    options: Vec<CommandDataOption>,
    /// If the option is not a subcommand or subcommand group this will contain the resolved value of the option.
    value: Option<CommandDataOptionValue>,
}

impl CommandOptions {
    /// Given the name of the parameter, returns the value of the argument
    /// as provided by the user if it exists.
    ///
    /// # Errors
    /// - If the option specified is a subcommand.
    /// - If the option does not exist.
    /// - If the option is optional and there is no value.
    pub fn get_value_by_name(&self, name: &str) -> Option<CommandDataOptionValue> {
        self.args
            .iter()
            .find(|arg| arg.name == name)?
            .value.clone()
    }

    /// Given the name of the parameter, returns the value of the argument
    /// as provided by the user if it exists as a String.
    ///
    /// # Errors
    /// - If the option specified is not a string with `Some(Err)`.
    /// - If the option does not exist with `None`.
    /// - If the option is optional and there is no value with `None`.
    pub fn get_string_value(&self, name: &str) -> Option<Result<String>> {
        let t = self.get_value_by_name(name)?;
        if let CommandDataOptionValue::String(s) = t {
            Some(Ok(s))
        } else {
            Some(Err(anyhow!("Option {} is not a string.", name)))
        }
    }

    /// Given the name of the parameter, returns the value of the argument
    /// as provided by the user if it exists as a boolean.
    ///
    /// # Errors
    /// - If the option specified is not a boolean with `Some(Err)`.
    /// - If the option does not exist with `None`.
    /// - If the option is optional and there is no value with `None`.
    pub fn get_bool_value(&self, name: &str) -> Option<Result<bool>> {
        let t = self.get_value_by_name(name)?;
        if let CommandDataOptionValue::Boolean(b) = t {
            Some(Ok(b))
        } else {
            Some(Err(anyhow!("Option {} is not a boolean.", name)))
        }
    }

    /// Given the name of the parameter, returns the value of the argument
    /// as provided by the user if it exists as an integer or a number.
    ///
    /// # Errors
    /// - If the option specified is not an integer or number with `Some(Err)`.
    /// - If the option does not exist with `None`.
    /// - If the option is optional and there is no value with `None`.
    pub fn get_int_or_number_value(&self, name: &str) -> Option<Result<IntOrNumber>> {
        let t = self.get_value_by_name(name)?;
        if let CommandDataOptionValue::Number(n) = t {
            Some(Ok(IntOrNumber::Number(n)))
        } else if let CommandDataOptionValue::Integer(i) = t {
            Some(Ok(IntOrNumber::Int(i)))
        } else {
            Some(Err(anyhow!("Option {} is not a number.", name)))
        }
    }

    /// Given the name of the parameter, returns the value of the argument
    /// as provided by the user if it exists as a user.
    ///
    /// # Errors
    /// - If the option specified is not a user with `Some(Err)`.
    /// - If the option does not exist with `None`.
    /// - If the option is optional and there is no value with `None`.
    pub fn get_user_value(&self, name: &str) -> Option<Result<(User, Option<PartialMember>)>> {
        let t = self.get_value_by_name(name)?;
        if let CommandDataOptionValue::User(u, m) = t {
            Some(Ok((u, m)))
        } else {
            Some(Err(anyhow!("Option {} is not a user.", name)))
        }
    }

    /// Given the name of the parameter, returns the value of the argument
    /// as provided by the user if it exists as a role.
    ///
    /// # Errors
    /// - If the option specified is not a role with `Some(Err)`.
    /// - If the option does not exist with `None`.
    /// - If the option is optional and there is no value with `None`.
    pub fn get_role_value(&self, name: &str) -> Option<Result<Role>> {
        let t = self.get_value_by_name(name)?;
        if let CommandDataOptionValue::Role(r) = t {
            Some(Ok(r))
        } else {
            Some(Err(anyhow!("Option {} is not a role.", name)))
        }
    }

    /// Given the name of the parameter, returns the value of the argument
    /// as provided by the user if it exists as a channel.
    ///
    /// # Errors
    /// - If the option specified is not a channel with `Some(Err)`.
    /// - If the option does not exist with `None`.
    /// - If the option is optional and there is no value with `None`.
    pub fn get_channel_value(&self, name: &str) -> Option<Result<PartialChannel>> {
        let t = self.get_value_by_name(name)?;
        if let CommandDataOptionValue::Channel(c) = t {
            Some(Ok(c))
        } else {
            Some(Err(anyhow!("Option {} is not a channel.", name)))
        }
    }

    /// Returns the name of the subcommand or subcommand group and its options.
    ///
    /// # Errors
    /// - If there is more or less than one option.
    /// - If the only option is not a subcommand or subcommand group.
    pub fn get_subcommand_args_and_name(&self) -> Option<(String, Self)> {
        if self.args.len() != 1 {
            return None;
        }
        let first_option = self.args.first()?.clone();
        if
            first_option.kind != CommandOptionType::SubCommand &&
            first_option.kind != CommandOptionType::SubCommandGroup
        {
            return None;
        }
        Some((first_option.name, first_option.options.into()))
    }
}

impl From<Vec<CommandDataOption>> for CommandOptions {
    fn from(args: Vec<CommandDataOption>) -> Self {
        let cmd_args = args
            .into_iter()
            .map(|arg| Opt {
                name: arg.name,
                kind: arg.kind,
                options: arg.options,
                value: arg.resolved,
            })
            .collect::<Vec<_>>();
        Self { args: cmd_args }
    }
}
