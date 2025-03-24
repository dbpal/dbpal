TRUNCATE TABLE table_name
26 employees
select id from employees
10 id ; 25 employees
select id from (select id from employees) as xxx
10 id
SELECT e.id AS EmployeeID,  e.name AS EmployeeName,  d.name AS DepartmentName FROM employees AS e JOIN departments AS d ON e.department_id = d.id;
9 e ; 9 employees ; 12 name ; 54 d ; 54 departments ; 60 name ; 93 departments ; 125 e ; 125 departments ; 139 department_id ; 143 d ; 146 id
select * from test.dbo.employees
19 test ; 23 dbo ; 31 employees
select count(*) from employees
11 count ; 11 abs ; 11 sin
insert into employees (id) select id from employees
18 employees ; 26 name ; 37 name
insert into employees (id) values (1)
26 name
update employees set name = 1 where id = 10000
25 name ; 25 id ; 39 name
delete from employees where id = 1000
31 id ; 31 name
create view view_name as select * from employees
49 employees
CREATE TABLE Employees (  Age INT, CONSTRAINT chk_age CHECK (Age >= 18));
65 Age
CREATE TABLE Orders ( CustomerID INT, OrderID INT, OrderDate DATETIME, CONSTRAINT pk_orders PRIMARY KEY (CustomerID, OrderID));
116 CustomerID ; 125 OrderID
CREATE TABLE Orders (  CustomerID INT, CONSTRAINT fk_customer FOREIGN KEY (CustomerID) REFERENCES employees (id));
86 CustomerID ; 108 employees ; 112 id
CREATE INDEX idx_shipped_orders ON employees (id) WHERE id = '100';
45 employees ; 49 id ; 59 id
ALTER TABLE employees ADD CONSTRAINT PK_Employees PRIMARY KEY (id);
22 employees ; 66 id
ALTER TABLE employees ADD CONSTRAINT xxxxx FOREIGN KEY (department_id) REFERENCES departments (id);
22 employees ; 70 department_id ; 94 departments ; 98 id
ALTER TABLE employees ADD CONSTRAINT chk_id CHECK (id >= 18);
54 id
ALTER TABLE employees ADD CONSTRAINT idx_lastname UNIQUE (id);
61 id
GRANT SELECT ON dbo.employees TO public;
20 dbo ; 30 employees
drop table employees;
21 employees
DROP FUNCTION test_function;
25 test_udf
DROP view employees_view;
25 test_view
explain  select * from employees
33 employees