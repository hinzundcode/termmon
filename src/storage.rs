use chrono::prelude::*;
use serde::{Serialize, Deserialize};
use rusqlite::{params, Connection};

pub struct SqliteStorage {
	conn: Connection,
}

impl SqliteStorage {
	pub fn new(conn: Connection) -> Self {
		conn.execute("create table if not exists command (
			id integer primary key,
			session_id text not null,
			`index` integer,
			command text not null,
			pwd text not null,
			status integer,
			timestamp text not null
		)", []).unwrap();
		
		Self { conn }
	}
	
	pub fn insert_command(&self, command: &mut Command) {
		self.conn.execute("INSERT INTO command (
			session_id, `index`, command, pwd, status, timestamp
		) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
			params![
				command.session_id, command.index, command.command,
				command.pwd, command.status, command.timestamp
			]
		).unwrap();
		
		let id = self.conn.last_insert_rowid();
		command.id = id;
	}
	
	pub fn get_latest_commands(&self) -> Vec<Command> {
		let mut stmt = self.conn.prepare("SELECT * FROM command ORDER BY timestamp DESC LIMIT 500").unwrap();
		
		let iter = stmt.query_map([], |row| {
			Ok(Command {
				id: row.get(0).unwrap(),
				session_id: row.get(1).unwrap(),
				index: row.get(2).unwrap(),
				command: row.get(3).unwrap(),
				pwd: row.get(4).unwrap(),
				status: row.get(5).unwrap(),
				timestamp: row.get(6).unwrap(),
			})
		}).unwrap();
		
		return iter.collect::<Result<Vec<Command>,rusqlite::Error>>().unwrap();
	}
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Command {
	pub id: i64,
	pub session_id: String,
	pub index: u32,
	pub command: String,
	pub pwd: String,
	pub status: u32,
	pub timestamp: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
	use super::*;
	
	#[test]
	fn test_insert() {
		let conn = Connection::open_in_memory().unwrap();
		let storage = SqliteStorage::new(conn);
		
		let mut command = Command {
			id: 0,
			session_id: "session_id".to_string(),
			index: 1,
			command: "echo foo".to_string(),
			pwd: "/".to_string(),
			status: 0,
			timestamp: Utc::now(),
		};
		
		storage.insert_command(&mut command);
		
		assert_eq!(command.id, 1);
	}
	
	#[test]
	fn test_get() {
		let conn = Connection::open_in_memory().unwrap();
		let storage = SqliteStorage::new(conn);
		
		let mut command1 = Command {
			id: 0,
			session_id: "session_id".to_string(),
			index: 1,
			command: "echo foo".to_string(),
			pwd: "/".to_string(),
			status: 0,
			timestamp: Utc::now(),
		};
		let mut command2 = Command {
			id: 0,
			session_id: "session_id".to_string(),
			index: 2,
			command: "echo bar".to_string(),
			pwd: "/".to_string(),
			status: 0,
			timestamp: Utc::now(),
		};
		
		storage.insert_command(&mut command1);
		storage.insert_command(&mut command2);
		
		let commands = storage.get_latest_commands();
		
		assert_eq!(commands, vec![command2, command1]);
	}
}
