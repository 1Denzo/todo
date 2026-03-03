use colored::*;
use std::fs;
use std::fs::OpenOptions;
use std::io::prelude::Read;
use std::io::{self, BufReader, BufWriter, Write};
use std::path::Path;
use std::{env, process};
use chrono::Local;

pub struct Entry {
    pub todo_entry: String,
    pub done: bool,
    pub created_at: Option<String>,
    pub completed_at: Option<String>,
}

impl Entry {
    pub fn new(todo_entry: String, done: bool) -> Self {
        let now = Local::now().format("%d.%m.%Y %H:%M:%S").to_string();
        Self {
            todo_entry,
            done,
            created_at: Some(now),
            completed_at: None,
        }
    }

    // Для обратной совместимости - создает запись без временных меток
    pub fn new_without_dates(todo_entry: String, done: bool) -> Self {
        Self {
            todo_entry,
            done,
            created_at: None,
            completed_at: None,
        }
    }

    pub fn file_line(&self) -> String {
        let symbol = if self.done { "[*] " } else { "[ ] " };
        
        // Формируем строку с временными метками, если они есть
        let mut meta_parts = Vec::new();
        
        if let Some(created) = &self.created_at {
            meta_parts.push(format!("created:{}", created));
        }
        
        if let Some(completed) = &self.completed_at {
            meta_parts.push(format!("completed:{}", completed));
        }
        
        if meta_parts.is_empty() {
            format!("{}{}\n", symbol, self.todo_entry)
        } else {
            format!("{}{} [{}]\n", symbol, self.todo_entry, meta_parts.join("; "))
        }
    }

    pub fn list_line(&self, number: usize) -> String {
        let mut todo_entry = if self.done {
            self.todo_entry.strikethrough().to_string()
        } else {
            self.todo_entry.clone()
        };

        // Добавляем информацию о времени создания, если она есть
        if let Some(created) = &self.created_at {
            todo_entry = format!("{} (создано: {})", todo_entry, created.dimmed());
        }

        // Добавляем информацию о времени выполнения, если задача выполнена
        if self.done {
            if let Some(completed) = &self.completed_at {
                todo_entry = format!("{} (выполнено: {})", todo_entry, completed.dimmed());
            }
        }

        format!("{number} {todo_entry}\n")
    }

    pub fn read_line(line: &String) -> Self {
        let done = line.starts_with("[*] ");
        
        // Проверяем, есть ли в строке метаданные с временными метками
        if line.contains(" [created:") || line.contains(" [completed:") {
            // Ищем начало метаданных
            if let Some(meta_start) = line.find(" [") {
                let todo_part = &line[4..meta_start]; // Пропускаем "[*] " или "[ ] "
                let meta_part = &line[meta_start + 2..line.len() - 1]; // -1 чтобы убрать закрывающую скобку
                
                let mut created_at = None;
                let mut completed_at = None;
                
                // Разбираем метаданные
                for meta in meta_part.split("; ") {
                    if let Some(created_val) = meta.strip_prefix("created:") {
                        created_at = Some(created_val.to_string());
                    } else if let Some(completed_val) = meta.strip_prefix("completed:") {
                        completed_at = Some(completed_val.to_string());
                    }
                }
                
                return Self {
                    todo_entry: todo_part.to_string(),
                    done,
                    created_at,
                    completed_at,
                };
            }
        }
        
        // Старый формат без временных меток
        Self {
            todo_entry: (&line[4..]).to_string(),
            done,
            created_at: None,
            completed_at: None,
        }
    }

    pub fn raw_line(&self) -> String {
        format!("{}\n", self.todo_entry)
    }
    
    // Отмечает задачу как выполненную и добавляет время выполнения
    pub fn mark_done(&mut self) {
        self.done = true;
        let now = Local::now().format("%d.%m.%Y %H:%M:%S").to_string();
        self.completed_at = Some(now);
    }
    
    // Переключает статус задачи
    pub fn toggle_done(&mut self) {
        if !self.done {
            // Если задача не была выполнена, отмечаем как выполненную с временем
            self.done = true;
            let now = Local::now().format("%d.%m.%Y %H:%M:%S").to_string();
            self.completed_at = Some(now);
        } else {
            // Если задача была выполнена, снимаем отметку и удаляем время выполнения
            self.done = false;
            self.completed_at = None;
        }
    }
}

pub struct Todo {
    pub todo: Vec<String>,
    pub todo_path: String,
    pub todo_bak: String,
    pub no_backup: bool,
}

impl Todo {
    pub fn new() -> Result<Self, String> {
        let todo_path: String = match env::var("TODO_PATH") {
            Ok(t) => t,
            Err(_) => {
                let home = env::var("HOME").unwrap();

                // Look for a legacy TODO file path
                let legacy_todo = format!("{}/TODO", &home);
                match Path::new(&legacy_todo).exists() {
                    true => legacy_todo,
                    false => format!("{}/.todo", &home),
                }
            }
        };

        let todo_bak: String = match env::var("TODO_BAK_DIR") {
            Ok(t) => t,
            Err(_) => String::from("/tmp/todo.bak"),
        };

        let no_backup = env::var("TODO_NOBACKUP").is_ok();

        let todofile = OpenOptions::new()
            .write(true)
            .read(true)
            .create(true)
            .open(&todo_path)
            .expect("Couldn't open the todofile");

        // Creates a new buf reader
        let mut buf_reader = BufReader::new(&todofile);

        // Empty String ready to be filled with TODOs
        let mut contents = String::new();

        // Loads "contents" string with data
        buf_reader.read_to_string(&mut contents).unwrap();

        // Splits contents of the TODO file into a todo vector
        let todo = contents.lines().map(str::to_string).collect();

        // Returns todo
        Ok(Self {
            todo,
            todo_path,
            todo_bak,
            no_backup,
        })
    }

    // Prints every todo saved
    pub fn list(&self) {
        let stdout = io::stdout();
        // Buffered writer for stdout stream
        let mut writer = BufWriter::new(stdout);
        let mut data = String::new();
        // This loop will repeat itself for each task in TODO file
        for (number, task) in self.todo.iter().enumerate() {
            let entry = Entry::read_line(task);

            let number = number + 1;

            let line = entry.list_line(number);
            data.push_str(&line);
        }
        writer
            .write_all(data.as_bytes())
            .expect("Failed to write to stdout");
    }

    // This one is for yall, dmenu chads <3
    pub fn raw(&self, arg: &[String]) {
        if arg.len() > 1 {
            eprintln!("todo raw takes only 1 argument, not {}", arg.len())
        } else if arg.is_empty() {
            eprintln!("todo raw takes 1 argument (done/todo)");
        } else {
            let stdout = io::stdout();
            // Buffered writer for stdout stream
            let mut writer = BufWriter::new(stdout);
            let mut data = String::new();
            let arg = &arg[0];
            // This loop will repeat itself for each task in TODO file
            for task in self.todo.iter() {
                let entry = Entry::read_line(task);
                if entry.done && arg == "done" {
                    data = entry.raw_line();
                } else if !entry.done && arg == "todo" {
                    data = entry.raw_line();
                }
                
                writer
                    .write_all(data.as_bytes())
                    .expect("Failed to write to stdout");
            }
        }
    }
    // Adds a new todo
    pub fn add(&self, args: &[String]) {
        if args.is_empty() {
            eprintln!("todo add takes at least 1 argument");
            process::exit(1);
        }
        // Opens the TODO file with a permission to:
        let todofile = OpenOptions::new()
            .create(true) // a) create the file if it does not exist
            .append(true) // b) append a line to it
            .open(&self.todo_path)
            .expect("Couldn't open the todofile");

        let mut buffer = BufWriter::new(todofile);
        for arg in args {
            if arg.trim().is_empty() {
                continue;
            }

            // Appends a new task/s to the file с временем создания
            let entry = Entry::new(arg.to_string(), false);
            let line = entry.file_line();
            buffer
                .write_all(line.as_bytes())
                .expect("unable to write data");
        }
    }

    // Removes a task
    pub fn remove(&self, args: &[String]) {
        if args.is_empty() {
            eprintln!("todo rm takes at least 1 argument");
            process::exit(1);
        }
        // Opens the TODO file with a permission to:
        let todofile = OpenOptions::new()
            .write(true) // a) write
            .truncate(true) // b) truncrate
            .open(&self.todo_path)
            .expect("Couldn't open the todo file");

        let mut buffer = BufWriter::new(todofile);

        for (pos, line) in self.todo.iter().enumerate() {
            if args.contains(&(pos + 1).to_string()) {
                continue;
            }

            let line = format!("{}\n", line);

            buffer
                .write_all(line.as_bytes())
                .expect("unable to write data");
        }
    }

    fn remove_file(&self) {
        match fs::remove_file(&self.todo_path) {
            Ok(_) => {}
            Err(e) => {
                eprintln!("Error while clearing todo file: {}", e)
            }
        };
    }
    // Clear todo by removing todo file
    pub fn reset(&self) {
        if !self.no_backup {
            match fs::copy(&self.todo_path, &self.todo_bak) {
                Ok(_) => self.remove_file(),
                Err(_) => {
                    eprint!("Couldn't backup the todo file")
                }
            }
        } else {
            self.remove_file();
        }
    }
    pub fn restore(&self) {
        fs::copy(&self.todo_bak, &self.todo_path).expect("unable to restore the backup");
    }

    // Sorts done tasks
    pub fn sort(&self) {
        // Creates a new empty string
        let newtodo: String;

        let mut todo = String::new();
        let mut done = String::new();

        for line in self.todo.iter() {
            let entry = Entry::read_line(line);
            if entry.done {
                let line = format!("{}\n", line);
                done.push_str(&line);
            } else {
                let line = format!("{}\n", line);
                todo.push_str(&line);
            }
        }

        newtodo = format!("{}{}", &todo, &done);
        // Opens the TODO file with a permission to:
        let mut todofile = OpenOptions::new()
            .write(true) // a) write
            .truncate(true) // b) truncrate
            .open(&self.todo_path)
            .expect("Couldn't open the todo file");

        // Writes contents of a newtodo variable into the TODO file
        todofile
            .write_all(newtodo.as_bytes())
            .expect("Error while trying to save the todofile");
    }

    pub fn done(&self, args: &[String]) {
        if args.is_empty() {
            eprintln!("todo done takes at least 1 argument");
            process::exit(1);
        }
        
        // Opens the TODO file with a permission to overwrite it
        let todofile = OpenOptions::new()
            .truncate(true)
            .create(true)
            .write(true)
            .open(&self.todo_path)
            .expect("Couldn't open the todofile");
        let mut buffer = BufWriter::new(todofile);
        let mut data = String::new();

        for (pos, line) in self.todo.iter().enumerate() {
            let mut entry = Entry::read_line(line);
            let line = if args.contains(&(pos + 1).to_string()) {
                // Используем toggle_done для правильной обработки временных меток
                entry.toggle_done();
                entry.file_line()
            } else {
                format!("{}\n", line)
            };
            
            data.push_str(&line);
        }
        buffer
            .write_all(data.as_bytes())
            .expect("unable to write data"); 
    }

    pub fn edit(&self, args: &[String]) {
        if args.is_empty() || args.len() != 2{
            eprintln!("todo edit takes exact 2 arguments");
            process::exit(1);
        }
        // Opens the TODO file with a permission to overwrite it
        let todofile = OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(&self.todo_path)
            .expect("Couldn't open the todofile");
        let mut buffer = BufWriter::new(todofile);

        for (pos, line) in self.todo.iter().enumerate() {
            let line = if args[0] == (pos + 1).to_string() { 
                let mut entry = Entry::read_line(line);
                entry.todo_entry = args[1].clone();
                // Сохраняем существующие временные метки
                entry.file_line()
            } else {
                format!("{}\n", line)
            };
            buffer
                .write_all(line.as_bytes())
                .expect("unable to write data");
        }
    }
}

pub fn help() {
    // For readability
    let yellow_bg = String::from("Usage: todo [COMMAND] [ARGUMENTS]
Todo is a super fast and simple tasks organizer written in rust
Example: todo list
Available commands:
    - add [TASK/s]
        adds new task/s with creation timestamp
        Example: todo add \"buy carrots\"
    - edit [INDEX] [EDITED TASK/s]
        edits an existing task/s
        Example: todo edit 1 banana
    - list
        lists all tasks with timestamps
        Example: todo list
    - done [INDEX]
        marks task as done with completion timestamp
        Example: todo done 2 3 (marks second and third tasks as completed)
    - rm [INDEX]
        removes a task
        Example: todo rm 4
    - reset
        deletes all tasks
    - restore 
        restore recent backup after reset
    - sort
        sorts completed and uncompleted tasks
        Example: todo sort
    - raw [todo/done]
        prints nothing but done/incompleted tasks in plain text, useful for scripting
        Example: todo raw done").yellow();
    // println!("{}", TODO_HELP);
    println!("{}", yellow_bg);
}
