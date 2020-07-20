let exg = require('../dist');

const express = require('express')
const app = express()
const port = 4000

exg.spawn(
    "01EG0GD5J8Q0N3DBJP7569RRFB",
    "2eoAYVjpjtztomf7mL94fJeZVS5TSkvEDSYB97v1CQxDDyfeg4ovcnuPq3CyY5Acs5pz95eufWb4o6Tpezg86um8RNoty613mZXSMWraLQRVqEtjzQ8RUU5eQDoSnhtvtpJy9htp67AYaJpiwMPmr1XgviUyVhNnsqLwYvZfV4ri1hUcihbW6E8zkeHyJ",
    "glebpom",
    "home"
)

app.get('/', (req, res) => res.send('<html><body><h1>Hello from JS</h1></body></html>'))
app.listen(port, () => console.log(`Example app listening at http://localhost:${port}`))
