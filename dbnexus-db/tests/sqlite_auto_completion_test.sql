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
delete from employees where id = 1000
31 id ; 31 name
create view view_name as select * from employees
49 employees
CREATE TABLE products (price REAL CHECK(price > 0),   quantity INTEGER CHECK(quantity >= 0) );
46 price ; 86 quantity 
CREATE INDEX index_name ON employees (id)
40 id
alter table employees rename to employees2
22 employees
alter table employees rename column id to id2
39 id
drop table employees
21 employees
drop view employees_view
25 employees_view
drop index employees_name
26 employees_name
PRAGMA user_version;
12 user_version