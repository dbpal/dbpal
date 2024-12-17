analyze table employees
24 employees
TRUNCATE TABLE table_name
26 employees
select id from employees
10 id ; 25 employees
select id from (select id from employees) as xxx
10 id
SELECT e.id AS EmployeeID,  e.name AS EmployeeName,  d.name AS DepartmentName FROM employees AS e JOIN departments AS d ON e.department_id = d.id;
9 e ; 9 employees ; 12 name ; 54 d ; 54 departments ; 60 name ; 93 departments ; 125 e ; 125 departments ; 139 department_id ; 143 d ; 146 id
select count(*) from employees
11 count ; 11 abs ; 11 sin
update employees set name = 1 where id = 10000
25 name ; 25 id ; 39 name
delete from employees where id = 1000
31 id ; 31 name
create view view_name as select * from employees
49 employees
CREATE TABLE employees ( age INT,  CONSTRAINT chk_age CHECK (age > 0));
65 age
CREATE TABLE employees ( id INT NOT NULL, PRIMARY KEY (id));
58 id
CREATE TABLE xxx ( id INT NOT NULL, name VARCHAR(100), FOREIGN KEY (department_id) REFERENCES departments(id));
106 departments ; 109 id
CREATE TABLE articles ( title VARCHAR(255), content TEXT, FULLTEXT (title, content));
74 title ; 83 content
CREATE INDEX idx_orders_status ON employees (id DESC) INCLUDE (id) WHERE id = 'shipped';
44 employees ; 48 id ; 66 id ; 76 id
ALTER TABLE employees ADD CONSTRAINT unique_id UNIQUE (id);
22 employees ; 58 id
ALTER TABLE employees ADD CONSTRAINT fk_department FOREIGN KEY (department_id) REFERENCES departments(id);
102 departments ; 105 id
ALTER TABLE employees ADD CONSTRAINT chk_id CHECK (id > 0);
54 id
ALTER TABLE employees ADD INDEX idx_name (name);
47 id
ALTER TABLE employees ADD FULLTEXT INDEX idx_fulltext (name);
60 name
ALTER TABLE employees ADD xxx INT after id
22 employees ; 43 id
drop table employees;
21 employees
DROP FUNCTION test_function;
25 test_function
DROP view employees_view;
25 employees_view
FLUSH TABLES employees;
23 employees
explain  select * from employees
33 employees