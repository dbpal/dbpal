truncate employees 
19 employees
select employees.id  from employees
13 employees ; 19 name ; 32 departments
SELECT e.id AS EmployeeID,  e.name AS EmployeeName,  d.name AS DepartmentName FROM employees AS e JOIN departments AS d ON e.department_id = d.id;
9 e ; 9 employees ; 12 name ; 54 d ; 54 departments ; 60 name ; 93 departments ; 125 e ; 125 departments ; 139 department_id ; 143 d ; 146 id
SELECT e.id AS EmployeeID,e.name AS EmployeeName,d.name AS DepartmentName FROM employees AS e JOIN departments AS d USING (department_id);
135 department_id
select id from (select id from employees)
10 id ; 25 name
select id from employees e 
10 name
insert into employees (id) select id from employees
18 employees ; 26 name ; 37 name
insert into employees (id) values (1)
26 name
select count(*) from employees
11 count ; 11 abs ; 11 sin
update employees set name = 1 where id = 10000
25 name ; 25 id ; 39 name
create view view_name as select * from employees
49 employees
COPY employees(id) FROM '/path/to/myfile.csv' WITH (FORMAT csv);
15 employees ; 18 id
update employees set name = 1 where id = 10000
25 name ; 25 id ; 39 name
update employees set name = 1 where id = 10000 RETURNING id 
25 name ; 25 id ; 39 name ; 60 id
update employees set name = 1 from employees where id = 10000
45 employees
delete from employees where id = 1000
31 id ; 31 name
delete from employees USING employees
38 employees
create view view_name as select * from employees
49 employees